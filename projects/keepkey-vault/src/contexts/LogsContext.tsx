import React, { createContext, useContext, useState, ReactNode } from 'react';

interface LogsContextType {
  deviceStatusMessages: string[];
  addStatusMessage: (message: string) => void;
  clearStatusMessages: () => void;
}

const LogsContext = createContext<LogsContextType | undefined>(undefined);

export const useLogContext = () => {
  const context = useContext(LogsContext);
  if (context === undefined) {
    throw new Error('useLogContext must be used within a LogsProvider');
  }
  return context;
};

interface LogsProviderProps {
  children: ReactNode;
}

export const LogsProvider: React.FC<LogsProviderProps> = ({ children }) => {
  const [deviceStatusMessages, setDeviceStatusMessages] = useState<string[]>([]);

  const addStatusMessage = (message: string) => {
    setDeviceStatusMessages(prev => {
      const newMessages = [...prev, message];
      // Keep only the last 50 messages to avoid memory issues
      return newMessages.slice(-50);
    });
  };

  const clearStatusMessages = () => {
    setDeviceStatusMessages([]);
  };

  const value = {
    deviceStatusMessages,
    addStatusMessage,
    clearStatusMessages,
  };

  return (
    <LogsContext.Provider value={value}>
      {children}
    </LogsContext.Provider>
  );
}; 