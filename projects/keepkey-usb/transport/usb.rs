use super::Transport;
use core::{cmp::min, iter::repeat, time::Duration};
use rusb::{ConfigDescriptor, Device, DeviceHandle, UsbContext};
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

pub struct UsbTransport<T: UsbContext> {
    handle: Arc<Mutex<DeviceHandle<T>>>,
    in_endpoint_address: u8,
    out_endpoint_address: u8,
    in_packet_size: usize,
    out_packet_size: usize,
}

impl<T: UsbContext> UsbTransport<T> {
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

        // Detaching the kernel driver is required to access HID devices on Mac OS and Linux.
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
        let locked_handle = handle_arc_param.lock().map_err(|_| rusb::Error::Other)?;

        let interface = config_descriptor
            .interfaces()
            .nth(interface_index)
            .ok_or(rusb::Error::NotFound)?;
        locked_handle.claim_interface(interface.number())?;

        let mut interface_descriptors = interface.descriptors();
        let interface_descriptor = interface_descriptors.next().ok_or(rusb::Error::NotFound)?;
        locked_handle.set_alternate_setting(interface.number(), 0)?;

        let mut endpoint_descriptors = interface_descriptor.endpoint_descriptors();

        let in_endpoint_descriptor = endpoint_descriptors.next().ok_or(rusb::Error::NotFound)?;
        if in_endpoint_descriptor.direction() != rusb::Direction::In {
            return Err(rusb::Error::InvalidParam);
        }
        if in_endpoint_descriptor.transfer_type() != rusb::TransferType::Interrupt {
            return Err(rusb::Error::InvalidParam);
        }

        let out_endpoint_descriptor = endpoint_descriptors.next().ok_or(rusb::Error::NotFound)?;
        if out_endpoint_descriptor.direction() != rusb::Direction::Out {
            return Err(rusb::Error::InvalidParam);
        }
        if out_endpoint_descriptor.transfer_type() != rusb::TransferType::Interrupt {
            return Err(rusb::Error::InvalidParam);
        }

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
        let len = self.handle.lock().map_err(|_| rusb::Error::Other)?.read_interrupt(
            self.in_endpoint_address,
            &mut packet,
            timeout,
        )?;
        if len != self.in_packet_size {
            return Err(rusb::Error::Other);
        }
        if !(len >= 1 && packet[0] == b'?') {
            return Err(rusb::Error::Other);
        }
        buf.extend_from_slice(&packet[1..]);
        Ok(())
    }

    /* // Commenting out this problematic method for now
    fn control_transfer(&self, req_type: u8, req: u8, val: u16, idx: u16, data: &[u8]) -> Result<Vec<u8>, Error> {
        let device_handle = self.device_handle.as_ref().ok_or(Error::DeviceNotConnected)?;
        let handle = device_handle.lock().unwrap();
        let timeout = std::time::Duration::from_secs(5);
        let mut buf = vec![0u8; 256]; // Standard buffer, might need adjustment
        // ... existing code ...
    }
    */
}

macro_rules! since {
    ($started:expr, $timeout:expr) => {
        $timeout
            .checked_sub($started.elapsed())
            .filter(|x| *x >= Duration::from_millis(1))
            .ok_or(rusb::Error::Timeout)
    };
}

impl<T: UsbContext> Transport for UsbTransport<T> {
    type Error = rusb::Error;
    fn write(&mut self, msg: &[u8], timeout: Duration) -> Result<usize, Self::Error> {
        let started = Instant::now();
        let mut packet = Vec::<u8>::with_capacity(self.out_packet_size);
        for chunk in msg.chunks(self.out_packet_size - 1) {
            packet.clear();
            packet.push(b'?');
            packet.extend_from_slice(chunk);
            packet.extend(repeat(0).take(self.out_packet_size - packet.len()));
            if packet.len() != self.out_packet_size {
                return Err(rusb::Error::Other);
            }

            let written_len = self.handle.lock().map_err(|_| rusb::Error::Other)?.write_interrupt(
                self.out_endpoint_address,
                &packet,
                since!(started, timeout)?,
            )?;
            if written_len != packet.len() {
                return Err(rusb::Error::Other);
            }
        }
        Ok(msg.len())
    }
    fn read(&mut self, buf: &mut Vec<u8>, timeout: Duration) -> Result<(), Self::Error> {
        let mut packet = Vec::<u8>::with_capacity(self.in_packet_size);
        let started = Instant::now();
        self.read_packet(&mut packet, timeout)?;

        if !(packet.len() >= 8 && packet[0] == b'#' && packet[1] == b'#') {
            return Err(rusb::Error::Other);
        }
        let msg_len: usize = u32::from_be_bytes(packet[4..8].try_into().map_err(|_| rusb::Error::Other)?)
            .try_into()
            .map_err(|_| rusb::Error::Other)?;

        let mut len_remaining = 8 + msg_len;
        loop {
            buf.extend_from_slice(&packet[..min(len_remaining, packet.len())]);
            len_remaining = len_remaining.saturating_sub(packet.len());

            if len_remaining == 0 {
                break;
            }

            packet.clear();
            self.read_packet(&mut packet, since!(started, timeout)?)?;
        }

        Ok(())
    }
    fn reset(&mut self) -> Result<(), Self::Error> {
        // Super hacky, but libusb (and thus rusb) don't have a convenient interface to flush the read buffer, or even a way to tell it's empty other than hitting a timeout
        const RESET_TIMEOUT: Duration = Duration::from_millis(10);
        let mut buf = vec![0u8; self.in_packet_size];
        loop {
            match self.handle.lock().map_err(|_| rusb::Error::Other)?.read_interrupt(
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
