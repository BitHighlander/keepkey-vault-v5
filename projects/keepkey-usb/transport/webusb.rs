use super::Transport;
use core::time::Duration;
use rusb::{ConfigDescriptor, Device, DeviceHandle, UsbContext};
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

/// WebUSB transport for modern KeepKey devices (firmware 7.10.0+)
/// Uses bulk endpoints instead of interrupt endpoints and different protocol framing
pub struct WebUsbTransport<T: UsbContext> {
    handle: Arc<Mutex<DeviceHandle<T>>>,
    in_endpoint_address: u8,
    out_endpoint_address: u8,
    in_packet_size: usize,
    out_packet_size: usize,
}

impl<T: UsbContext> WebUsbTransport<T> {
    #[allow(clippy::type_complexity)]
    pub fn new(
        device: &Device<T>,
        interface_index: usize,
    ) -> Result<(Self, ConfigDescriptor, Arc<Mutex<DeviceHandle<T>>>), rusb::Error> {
        let config_descriptor = device.active_config_descriptor()?;
        let mut handle_arc = Arc::new(Mutex::new(device.open()?));

        let locked_handle = Arc::get_mut(&mut handle_arc)
            .ok_or(rusb::Error::Other)?
            .get_mut()
            .map_err(|_| rusb::Error::Other)?;
        
        locked_handle.reset()?;

        // WebUSB devices don't need kernel driver detaching like HID devices
        // But we'll still set auto-detach just in case
        match locked_handle.set_auto_detach_kernel_driver(true) {
            Err(rusb::Error::NotSupported) => Ok(()),
            x => x,
        }?;

        Ok((
            Self::new_from_descriptor_and_handle(
                &config_descriptor,
                Arc::clone(&handle_arc),
                interface_index,
            )?,
            config_descriptor,
            handle_arc,
        ))
    }

    pub fn new_from_descriptor_and_handle(
        config_descriptor: &ConfigDescriptor,
        handle_arc_param: Arc<Mutex<DeviceHandle<T>>>,
        interface_index: usize,
    ) -> Result<Self, rusb::Error> {
        println!("🔧 WebUSB: Creating transport from descriptor and handle...");
        
        let locked_handle = handle_arc_param.lock().map_err(|_| rusb::Error::Other)?;

        let interface = config_descriptor
            .interfaces()
            .nth(interface_index)
            .ok_or(rusb::Error::NotFound)?;
        
        println!("🔧 WebUSB: Found interface {}", interface.number());
        locked_handle.claim_interface(interface.number())?;

        let mut interface_descriptors = interface.descriptors();
        let interface_descriptor = interface_descriptors.next().ok_or(rusb::Error::NotFound)?;
        locked_handle.set_alternate_setting(interface.number(), 0)?;

        let mut endpoint_descriptors = interface_descriptor.endpoint_descriptors();
        let endpoints: Vec<_> = endpoint_descriptors.collect();
        
        println!("🔧 WebUSB: Found {} endpoints in interface", endpoints.len());
        for (i, ep) in endpoints.iter().enumerate() {
            println!("   WebUSB Endpoint {}: addr=0x{:02x}, type={:?}, dir={:?}", 
                     i, ep.address(), ep.transfer_type(), ep.direction());
        }

        // WebUSB uses BULK endpoints, not interrupt
        let mut endpoint_descriptors = interface_descriptor.endpoint_descriptors();
        let in_endpoint_descriptor = endpoint_descriptors.next().ok_or(rusb::Error::NotFound)?;
        
        println!("🔧 WebUSB: Checking first endpoint - dir={:?}, type={:?}", 
                 in_endpoint_descriptor.direction(), in_endpoint_descriptor.transfer_type());
        
        if in_endpoint_descriptor.direction() != rusb::Direction::In {
            println!("❌ WebUSB: First endpoint is not IN direction");
            return Err(rusb::Error::InvalidParam);
        }
        if in_endpoint_descriptor.transfer_type() != rusb::TransferType::Bulk {
            println!("❌ WebUSB: First endpoint is not BULK type (got {:?})", in_endpoint_descriptor.transfer_type());
            return Err(rusb::Error::InvalidParam);
        }

        let out_endpoint_descriptor = endpoint_descriptors.next().ok_or(rusb::Error::NotFound)?;
        
        println!("🔧 WebUSB: Checking second endpoint - dir={:?}, type={:?}", 
                 out_endpoint_descriptor.direction(), out_endpoint_descriptor.transfer_type());
        
        if out_endpoint_descriptor.direction() != rusb::Direction::Out {
            println!("❌ WebUSB: Second endpoint is not OUT direction");
            return Err(rusb::Error::InvalidParam);
        }
        if out_endpoint_descriptor.transfer_type() != rusb::TransferType::Bulk {
            println!("❌ WebUSB: Second endpoint is not BULK type (got {:?})", out_endpoint_descriptor.transfer_type());
            return Err(rusb::Error::InvalidParam);
        }

        println!("✅ WebUSB: Successfully validated bulk endpoints");
        println!("   IN endpoint: 0x{:02x} (packet size: {})", 
                 in_endpoint_descriptor.address(), in_endpoint_descriptor.max_packet_size());
        println!("   OUT endpoint: 0x{:02x} (packet size: {})", 
                 out_endpoint_descriptor.address(), out_endpoint_descriptor.max_packet_size());

        drop(locked_handle);

        Ok(Self {
            handle: handle_arc_param,
            in_endpoint_address: in_endpoint_descriptor.address(),
            out_endpoint_address: out_endpoint_descriptor.address(),
            in_packet_size: in_endpoint_descriptor.max_packet_size().into(),
            out_packet_size: out_endpoint_descriptor.max_packet_size().into(),
        })
    }

    fn read_packet(&self, buf: &mut Vec<u8>, timeout: Duration) -> Result<(), rusb::Error> {
        let mut packet = vec![0u8; self.in_packet_size];
        let len = self.handle.lock().map_err(|_| rusb::Error::Other)?.read_bulk(
            self.in_endpoint_address,
            &mut packet,
            timeout,
        )?;
        
        // WebUSB doesn't use the '?' magic byte prefix like HID
        // Just append the received data directly
        buf.extend_from_slice(&packet[..len]);
        Ok(())
    }
}

macro_rules! since {
    ($started:expr, $timeout:expr) => {
        $timeout
            .checked_sub($started.elapsed())
            .filter(|x| *x >= Duration::from_millis(1))
            .ok_or(rusb::Error::Timeout)
    };
}

impl<T: UsbContext> Transport for WebUsbTransport<T> {
    type Error = rusb::Error;
    
    fn write(&mut self, msg: &[u8], timeout: Duration) -> Result<usize, Self::Error> {
        let started = Instant::now();
        
        // WebUSB protocol: send data directly without '?' prefix
        // The message should already be properly framed by the protocol layer
        let written_len = self.handle.lock().map_err(|_| rusb::Error::Other)?.write_bulk(
            self.out_endpoint_address,
            msg,
            since!(started, timeout)?,
        )?;
        
        if written_len != msg.len() {
            return Err(rusb::Error::Other);
        }
        
        Ok(msg.len())
    }
    
    fn read(&mut self, buf: &mut Vec<u8>, timeout: Duration) -> Result<(), Self::Error> {
        let started = Instant::now();
        
        // For WebUSB, we read the complete message directly
        // The protocol layer handles message framing
        let mut temp_buf = vec![0u8; 64]; // Standard WebUSB packet size
        let len = self.handle.lock().map_err(|_| rusb::Error::Other)?.read_bulk(
            self.in_endpoint_address,
            &mut temp_buf,
            timeout,
        )?;
        
        buf.extend_from_slice(&temp_buf[..len]);
        Ok(())
    }
    
    fn reset(&mut self) -> Result<(), Self::Error> {
        // Flush any pending data from the bulk endpoints
        const RESET_TIMEOUT: Duration = Duration::from_millis(10);
        let mut buf = vec![0u8; self.in_packet_size];
        
        loop {
            match self.handle.lock().map_err(|_| rusb::Error::Other)?.read_bulk(
                self.in_endpoint_address,
                &mut buf,
                RESET_TIMEOUT,
            ) {
                Ok(0) => return Ok(()),
                Ok(_) => (),
                Err(rusb::Error::Timeout) => return Ok(()),
                Err(rusb::Error::Overflow) => (),
                Err(x) => return Err(x),
            }
            buf.fill(0);
        }
    }
} 