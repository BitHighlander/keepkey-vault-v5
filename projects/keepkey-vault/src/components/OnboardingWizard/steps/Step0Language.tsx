import {
  Box,
  Button,
  Card,
  HStack,
  Text,
  VStack,
  Icon,
} from "@chakra-ui/react";
import { FaGlobe } from "react-icons/fa";
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

// Step components no longer need props - navigation handled by main wizard

const LANGUAGES = [
  { key: "en", label: "English" },
  { key: "es", label: "Español" },
  { key: "fr", label: "Français" },
  { key: "de", label: "Deutsch" },
  { key: "it", label: "Italiano" },
  { key: "pt", label: "Português" },
  { key: "ru", label: "Русский" },
  { key: "zh", label: "中文" },
  { key: "ja", label: "日本語" },
  { key: "ko", label: "한국어" },
];

export function Step0Language() {
  const [selectedLanguage, setSelectedLanguage] = useState("en");

  useEffect(() => {
    // Load saved language preference
    loadLanguagePreference();
  }, []);

  const loadLanguagePreference = async () => {
    try {
      const savedLang = await invoke<string | null>("get_preference", { key: "language" });
      if (savedLang) {
        setSelectedLanguage(savedLang);
      }
    } catch (error) {
      console.error("Failed to load language preference:", error);
    }
  };

  const handleLanguageChange = (event: React.ChangeEvent<HTMLSelectElement>) => {
    setSelectedLanguage(event.target.value);
    // Auto-save language preference when changed
    handleLanguageSave(event.target.value);
  };

  const handleLanguageSave = async (language: string) => {
    try {
      // Save language preference immediately
      await invoke("set_preference", { key: "language", value: language });
      console.log("Language preference saved:", language);
    } catch (error) {
      console.error("Failed to save language preference:", error);
    }
  };

  // Language preference is automatically saved when changed
  // Navigation is handled by the main wizard component

  return (
    <Box width="full" maxWidth="lg">
      <Card.Root bg="gray.900" borderColor="gray.700">
        <Card.Header bg="gray.850">
          <HStack justify="center" gap={3}>
            <Icon asChild color="green.500">
              <FaGlobe />
            </Icon>
            <Text fontSize="xl" fontWeight="bold" color="white">
              Select Your Language
            </Text>
          </HStack>
        </Card.Header>
        <Card.Body>
          <VStack gap={6}>
            <Text color="gray.400" textAlign="center">
              Choose your preferred language for the KeepKey Desktop application.
            </Text>
            
            <Box width="full" maxWidth="md">
              <Text color="white" fontSize="sm" mb={2} fontWeight="medium">
                Language / Idioma / Langue / Sprache
              </Text>
              <select
                value={selectedLanguage}
                onChange={handleLanguageChange}
                style={{
                  width: '100%',
                  padding: '0.75rem',
                  fontSize: '1rem',
                  borderRadius: '0.5rem',
                  border: '2px solid #4A5568',
                  backgroundColor: '#2D3748',
                  color: '#E2E8F0',
                  cursor: 'pointer'
                }}
              >
                {LANGUAGES.map((lang) => (
                  <option key={lang.key} value={lang.key} style={{ backgroundColor: '#2D3748' }}>
                    {lang.label}
                  </option>
                ))}
              </select>
            </Box>
            
            <Box textAlign="center" mt={4}>
              <Text color="green.400" fontSize="sm">
                ✓ Language will be applied immediately
              </Text>
            </Box>
          </VStack>
        </Card.Body>
      </Card.Root>
    </Box>
  );
} 