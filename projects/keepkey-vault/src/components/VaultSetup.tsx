import { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import {
  Box,
  Button,
  Input,
  VStack,
  Text,
  Spinner,
  HStack
} from '@chakra-ui/react'

export function VaultSetup() {
  const [vaultExists, setVaultExists] = useState(false)
  const [password, setPassword] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [success, setSuccess] = useState<string | null>(null)
  const [checking, setChecking] = useState(true)

  // Check if vault exists on mount
  useEffect(() => {
    checkVault()
  }, [])

  const checkVault = async () => {
    try {
      // const exists = await invoke<boolean>('check_vault_exists')
      const exists = true
      setVaultExists(exists)
    } catch (e) {
      console.error('Failed to check vault:', e)
    } finally {
      setChecking(false)
    }
  }

  const getKeepKeySignature = async (): Promise<string> => {
    // TODO: Implement signature retrieval from KeepKey device
    return "placeholder-signature";
  }

  const handleCreateVault = async () => {
    if (!password) {
      setError('Please enter a password')
      return
    }

    setLoading(true)
    setError(null)
    setSuccess(null)

    try {
      const signature = await getKeepKeySignature()
      await invoke('create_vault', {
        password,
        kkSignature: signature
      })
      setSuccess('Vault created successfully!')
      setVaultExists(true)
      setPassword('')
    } catch (e: any) {
      setError(e.toString())
    } finally {
      setLoading(false)
    }
  }

  const handleUnlockVault = async () => {
    if (!password) {
      setError('Please enter your password')
      return
    }

    setLoading(true)
    setError(null)
    setSuccess(null)

    try {
      const signature = await getKeepKeySignature()
      await invoke('unlock_vault', {
        password,
        kkSignature: signature
      })
      setSuccess('Vault unlocked successfully!')
      setPassword('')
    } catch (e: any) {
      setError(e.toString())
    } finally {
      setLoading(false)
    }
  }

  if (checking) {
    return (
      <Box textAlign="center" py={8}>
        <Spinner size="lg" color="blue.500" />
        <Text mt={4} color="gray.300">Checking vault status...</Text>
      </Box>
    )
  }

  return (
    <VStack align="stretch" gap={4}>
      <Text fontSize="lg" fontWeight="bold" color="white">
        Vault Setup
      </Text>

      {error && (
        <Box 
          p={3} 
          borderRadius="md" 
          bg="red.900" 
          color="white" 
          borderWidth="1px" 
          borderColor="red.700"
        >
          <Text>{error}</Text>
        </Box>
      )}

      {success && (
        <Box 
          p={3} 
          borderRadius="md" 
          bg="green.900" 
          color="white" 
          borderWidth="1px" 
          borderColor="green.700"
        >
          <Text>{success}</Text>
        </Box>
      )}

      <Box bg="gray.800" p={4} borderRadius="md">
        <Text color="gray.300" mb={4}>
          {vaultExists 
            ? 'A vault already exists. Enter your password to unlock it.'
            : 'No vault found. Create a new encrypted vault to store your data.'}
        </Text>

        <VStack align="stretch" gap={2}>
          <Text color="gray.300" fontSize="sm" fontWeight="medium">Password</Text>
          <Input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder="Enter your password"
            bg="gray.700"
            color="white"
            _placeholder={{ color: 'gray.500' }}
            onKeyPress={(e) => {
              if (e.key === 'Enter') {
                vaultExists ? handleUnlockVault() : handleCreateVault()
              }
            }}
          />
        </VStack>

        <HStack justify="flex-end" mt={4}>
          <Button
            colorScheme={vaultExists ? 'blue' : 'green'}
            onClick={vaultExists ? handleUnlockVault : handleCreateVault}
            loading={loading}
          >
            {vaultExists ? 'Unlock Vault' : 'Create Vault'}
          </Button>
        </HStack>
      </Box>

      <Text fontSize="sm" color="gray.500">
        Note: This will request a signature from your KeepKey device to derive the encryption key.
      </Text>
    </VStack>
  )
} 