package main

/*
#cgo CXXFLAGS: -I./steamworks_sdk_162/sdk/public
#cgo windows LDFLAGS: steam_init.o -L./steamworks_sdk_162/sdk/redistributable_bin/win64 -lsteam_api64
#cgo !windows LDFLAGS: steam_init.o
#include "steam_init.h"
*/
import "C"

import (
	"bufio"
	"encoding/base64"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"time"
	"unsafe"
)

func main() {
	// Set working directory to program directory for correct relative paths
	if exePath, err := os.Executable(); err == nil {
		exeDir := filepath.Dir(exePath)
		os.Chdir(exeDir)
	}

	// Display menu options
	fmt.Println("1. Extract Ticket")
	fmt.Println("2. Import Ticket")
	fmt.Print("Enter your choice (1-2): ")

	reader := bufio.NewReader(os.Stdin)
	choice, err := reader.ReadString('\n')
	if err != nil {
		fmt.Printf("❌ Failed to read input: %v\n", err)
		waitForExit()
		return
	}

	choice = strings.TrimSpace(choice)

	switch choice {
	case "1":
		extractTicket()
	case "2":
		importTicket()
	default:
		fmt.Println("❌ Invalid input, please try again")
	}

	waitForExit()
}

func extractTicket() {
	// Get App ID from user input
	fmt.Print("Enter the AppID: ")
	reader := bufio.NewReader(os.Stdin)
	input, err := reader.ReadString('\n')
	if err != nil {
		fmt.Printf("❌ Failed to read input: %v\n", err)
		return
	}

	input = strings.TrimSpace(input)
	appID, err := strconv.Atoi(input)
	if err != nil {
		fmt.Printf("❌ Invalid App ID: %v\n", err)
		return
	}

	// Set App ID using environment variables
	os.Setenv("SteamAppId", fmt.Sprintf("%d", appID))
	os.Setenv("SteamGameId", fmt.Sprintf("%d", appID))

	if !steamInit() {
		fmt.Println("❌ Steam initialization failed, please make sure Steam client is running")
		return
	}

	// Request encrypted app ticket
	if !requestEncryptedAppTicket() {
		fmt.Println("❌ Failed to request encrypted ticket, please check Steam connection")
		return
	}

	// Wait for ticket generation
	fmt.Println("⏳ Waiting for encrypted app ticket...")
	var ticket [2048]byte
	var tSize int

	// Wait up to 10 seconds
	for i := 0; i < 100; i++ {
		time.Sleep(100 * time.Millisecond)
		tSize = getEncryptedAppTicket(ticket[:])
		if tSize > 0 {
			break
		}
	}

	if tSize == 0 {
		fmt.Println("❌ Failed to get encrypted ticket, timeout after 10 seconds")
		return
	}

	// Convert to Base64
	ticketData := ticket[:tSize]
	ticketB64 := base64.StdEncoding.EncodeToString(ticketData)

	// Try to get Steam ID after ticket generation
	fmt.Println("🔍 Getting Steam ID...")
	steamID := getSteamID()

	if steamID != 0 {
		fmt.Printf("✅ Steam ID: %d\n", steamID)
	} else {
		fmt.Println("⚠️ Could not get Steam ID directly")
	}

	fmt.Printf("✅ Encrypted App Ticket (Base64): %s\n", ticketB64)

	// Read and update configs.user.ini file
	exeDir, err := os.Executable()
	if err != nil {
		fmt.Printf("❌ Failed to get program directory: %v\n", err)
		return
	}

	// Read from original config file
	originalConfigFile := filepath.Join(filepath.Dir(exeDir), "utils", "steam_settings", "configs.user.ini")
	content, err := os.ReadFile(originalConfigFile)
	if err != nil {
		fmt.Printf("❌ Failed to read config file %s: %v\n", originalConfigFile, err)
		return
	}

	// Write to App ID specific config file
	configFile := filepath.Join(filepath.Dir(exeDir), "utils", "steam_settings", fmt.Sprintf("configs.user.%d.ini", appID))

	lines := strings.Split(string(content), "\n")

	// Update corresponding lines
	steamIDUpdated := false
	ticketUpdated := false

	for i, line := range lines {
		trimmed := strings.TrimSpace(line)

		// Update Steam ID line
		if strings.HasPrefix(trimmed, "account_steamid=") || strings.HasPrefix(trimmed, "# account_steamid=") {
			lines[i] = fmt.Sprintf("account_steamid=%d", steamID)
			steamIDUpdated = true
		}

		// Update ticket line
		if strings.HasPrefix(trimmed, "ticket=") || strings.HasPrefix(trimmed, "# ticket=") {
			lines[i] = fmt.Sprintf("ticket=%s", ticketB64)
			ticketUpdated = true
		}
	}

	// Write file
	updatedContent := strings.Join(lines, "\n")
	err = os.WriteFile(configFile, []byte(updatedContent), 0644)
	if err != nil {
		fmt.Printf("❌ Failed to update config file %s: %v\n", configFile, err)
		return
	}

	fmt.Printf("✅ Config file updated successfully: %s\n", configFile)
	if steamIDUpdated {
		fmt.Printf("  - Steam ID: %d\n", steamID)
	}
	if ticketUpdated {
		fmt.Printf("  - Ticket updated\n")
	}
	if !steamIDUpdated && !ticketUpdated {
		fmt.Println("⚠️ No matching config lines found")
	}
}

func waitForExit() {
	fmt.Print("\nPress Enter to exit...")
	bufio.NewReader(os.Stdin).ReadLine()
}

func steamInit() bool {
	return bool(C.Steam_Init())
}

func requestEncryptedAppTicket() bool {
	return bool(C.RequestEncryptedAppTicket())
}

func getEncryptedAppTicket(ticket []byte) int {
	return int(C.GetEncryptedAppTicket(unsafe.Pointer(&ticket[0]), C.int(len(ticket))))
}

func getSteamID() uint64 {
	return uint64(C.GetSteamID())
}

func importTicket() {
	// Get App ID from user input
	fmt.Print("Enter the AppID: ")
	reader := bufio.NewReader(os.Stdin)
	input, err := reader.ReadString('\n')
	if err != nil {
		fmt.Printf("❌ Failed to read input: %v\n", err)
		return
	}

	input = strings.TrimSpace(input)
	appID, err := strconv.Atoi(input)
	if err != nil {
		fmt.Printf("❌ Invalid App ID: %v\n", err)
		return
	}

	fmt.Printf("🔍 Looking for AppID %d installation directory...\n", appID)

	// Get Steam installation path
	steamPath := getSteamPathFromRegistry()
	if steamPath == "" {
		fmt.Println("❌ Steam installation path not found")
		return
	}

	fmt.Printf("✅ Steam path found: %s\n", steamPath)

	// Parse library folders
	libraryPaths, err := parseLibraryFolders(steamPath)
	if err != nil {
		fmt.Printf("❌ Failed to parse library folders: %v\n", err)
		return
	}

	fmt.Printf("🔍 Found %d Steam libraries\n", len(libraryPaths))

	// Find game installation directory
	gameDir, err := findGameInstallDir(libraryPaths, fmt.Sprintf("%d", appID))
	if err != nil {
		fmt.Printf("❌ Failed to find game installation: %v\n", err)
		return
	}

	fmt.Printf("✅ Game found at: %s\n", gameDir)

	// Copy steamclient64 files
	err = copyGameFiles(gameDir)
	if err != nil {
		fmt.Printf("❌ Failed to copy game files: %v\n", err)
		return
	}

	// Process steam_api64.dll
	steamAPI64GamePath, err := processSteamAPI64(gameDir, fmt.Sprintf("%d", appID))
	if err != nil {
		fmt.Printf("❌ Failed to process steam_api64.dll: %v\n", err)
		return
	}

	// Initialize Steam and get Steam ID
	fmt.Println("🔍 Initializing Steam and getting Steam ID...")

	// Set App ID using environment variables
	os.Setenv("SteamAppId", fmt.Sprintf("%d", appID))
	os.Setenv("SteamGameId", fmt.Sprintf("%d", appID))

	if !steamInit() {
		fmt.Println("⚠️ Steam initialization failed, skipping Steam ID update")
	} else {
		steamID := getSteamID()
		if steamID == 0 {
			fmt.Println("⚠️ Could not get Steam ID, skipping config update")
		} else {
			fmt.Printf("✅ Steam ID: %d\n", steamID)

			// Update config file with alt_steamid
			err = updateConfigWithSteamID(fmt.Sprintf("%d", appID), steamID)
			if err != nil {
				fmt.Printf("❌ Failed to update config with Steam ID: %v\n", err)
				return
			}
		}
	}

	// Copy steam_settings files to game directory (same directory as steam_api64.dll)
	steamAPI64Dir := filepath.Dir(steamAPI64GamePath)
	err = copySteamSettingsToGame(steamAPI64Dir, fmt.Sprintf("%d", appID))
	if err != nil {
		fmt.Printf("❌ Failed to copy steam_settings: %v\n", err)
		return
	}

	fmt.Println("✅ Ticket import completed successfully!")

	// Wait for user to press Enter before exiting
	fmt.Print("\nPress Enter to exit...")
	bufio.NewReader(os.Stdin).ReadLine()
}

func getSteamPathFromRegistry() string {
	// Try to get from user registry
	if path := queryRegistry("HKCU", "SOFTWARE\\Valve\\Steam", "SteamPath"); path != "" {
		return path
	}

	// Try to get from system registry
	if path := queryRegistry("HKLM", "SOFTWARE\\WOW6432Node\\Valve\\Steam", "InstallPath"); path != "" {
		return path
	}

	return ""
}

func queryRegistry(hive, key, value string) string {
	cmd := exec.Command("reg", "query", hive+"\\"+key, "/v", value)
	output, err := cmd.Output()
	if err != nil {
		return ""
	}

	lines := strings.Split(string(output), "\n")
	for _, line := range lines {
		if strings.Contains(line, value) {
			parts := strings.Fields(line)
			if len(parts) >= 3 {
				// Get path part (may contain spaces)
				pathStart := strings.Index(line, "REG_SZ") + 6
				if pathStart > 6 && pathStart < len(line) {
					path := strings.TrimSpace(line[pathStart:])
					return strings.ReplaceAll(path, "/", "\\")
				}
			}
		}
	}

	return ""
}

func parseLibraryFolders(steamPath string) ([]string, error) {
	libraryFile := filepath.Join(steamPath, "steamapps", "libraryfolders.vdf")

	content, err := os.ReadFile(libraryFile)
	if err != nil {
		return nil, fmt.Errorf("failed to read libraryfolders.vdf: %v", err)
	}

	var paths []string
	lines := strings.Split(string(content), "\n")

	for _, line := range lines {
		line = strings.TrimSpace(line)
		if strings.Contains(line, "\"path\"") && strings.Count(line, "\"") >= 4 {
			parts := strings.Split(line, "\"")
			if len(parts) >= 4 {
				path := parts[3]
				// Convert forward slashes to backslashes for Windows
				path = strings.ReplaceAll(path, "\\\\", "\\")
				paths = append(paths, path)
				fmt.Printf("  📁 Library: %s\n", path)
			}
		}
	}

	return paths, nil
}

func findGameInstallDir(libraryPaths []string, appID string) (string, error) {
	for _, libraryPath := range libraryPaths {
		acfFile := filepath.Join(libraryPath, "steamapps", fmt.Sprintf("appmanifest_%s.acf", appID))

		if _, err := os.Stat(acfFile); os.IsNotExist(err) {
			continue
		}

		content, err := os.ReadFile(acfFile)
		if err != nil {
			continue
		}

		lines := strings.Split(string(content), "\n")
		for _, line := range lines {
			line = strings.TrimSpace(line)
			if strings.Contains(line, "\"installdir\"") && strings.Count(line, "\"") >= 4 {
				parts := strings.Split(line, "\"")
				if len(parts) >= 4 {
					installDir := parts[3]
					gameDir := filepath.Join(libraryPath, "steamapps", "common", installDir)
					return gameDir, nil
				}
			}
		}
	}

	return "", fmt.Errorf("game with AppID %s not found in any library", appID)
}

func copyGameFiles(gameDir string) error {
	// Get current executable directory
	exePath, err := os.Executable()
	if err != nil {
		return fmt.Errorf("failed to get executable path: %v", err)
	}
	exeDir := filepath.Dir(exePath)

	// Source directory containing steamclient64 files
	sourceDir := filepath.Join(exeDir, "utils", "gbe_fork")

	// Check if source directory exists
	if _, err := os.Stat(sourceDir); os.IsNotExist(err) {
		return fmt.Errorf("source directory not found: %s", sourceDir)
	}

	// Copy steamclient64.dll
	steamclient64Source := filepath.Join(sourceDir, "steamclient64.dll")
	steamclient64Dest := filepath.Join(gameDir, "steamclient64.dll")

	if _, err := os.Stat(steamclient64Source); os.IsNotExist(err) {
		return fmt.Errorf("steamclient64.dll not found in %s", sourceDir)
	}

	err = copyFile(steamclient64Source, steamclient64Dest)
	if err != nil {
		return fmt.Errorf("failed to copy steamclient64.dll: %v", err)
	}

	fmt.Printf("✅ Copied steamclient64.dll to %s\n", gameDir)
	return nil
}

func copyFile(src, dst string) error {
	sourceFile, err := os.Open(src)
	if err != nil {
		return err
	}
	defer sourceFile.Close()

	destFile, err := os.Create(dst)
	if err != nil {
		return err
	}
	defer destFile.Close()

	_, err = io.Copy(destFile, sourceFile)
	return err
}

func findSteamAPI64(gameDir string) (string, error) {
	var foundPath string
	foundError := fmt.Errorf("found")

	err := filepath.Walk(gameDir, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return nil // Continue walking even if there's an error with one file/dir
		}

		if !info.IsDir() && strings.ToLower(info.Name()) == "steam_api64.dll" {
			foundPath = path
			return foundError // Found it, stop walking
		}

		return nil
	})

	// If we got our special "found" error, that means we found the file
	if err == foundError {
		return foundPath, nil
	}

	if err != nil {
		return "", err
	}

	return "", fmt.Errorf("steam_api64.dll not found")
}

func processSteamAPI64(gameDir, appID string) (string, error) {
	// Get current executable directory
	exePath, err := os.Executable()
	if err != nil {
		return "", fmt.Errorf("failed to get executable path: %v", err)
	}
	exeDir := filepath.Dir(exePath)

	// Find steam_api64.dll in game directory and subdirectories
	fmt.Printf("🔍 Searching for steam_api64.dll in: %s\n", gameDir)
	steamAPI64GamePath, err := findSteamAPI64(gameDir)
	if err != nil {
		return "", fmt.Errorf("steam_api64.dll not found in game directory: %s", err)
	}

	fmt.Printf("✅ Found steam_api64.dll: %s\n", steamAPI64GamePath)

	// Create backup directory
	bakDir := filepath.Join(exeDir, "utils", "gbe_fork", "bak")
	if err := os.MkdirAll(bakDir, 0755); err != nil {
		return "", fmt.Errorf("failed to create backup directory: %v", err)
	}

	// Generate timestamp for backup
	timestamp := time.Now().Format("20060102_150405")
	backupFileName := fmt.Sprintf("steam_api64_%s_%s.dll", appID, timestamp)
	backupPath := filepath.Join(bakDir, backupFileName)

	// Backup original steam_api64.dll
	err = copyFile(steamAPI64GamePath, backupPath)
	if err != nil {
		return "", fmt.Errorf("failed to backup steam_api64.dll: %v", err)
	}
	fmt.Printf("✅ Backed up steam_api64.dll to: %s\n", backupPath)

	// Copy new steam_api64.dll from gbe_fork
	steamAPI64Source := filepath.Join(exeDir, "utils", "gbe_fork", "steam_api64.dll")
	if _, err := os.Stat(steamAPI64Source); os.IsNotExist(err) {
		return "", fmt.Errorf("steam_api64.dll not found in gbe_fork: %s", steamAPI64Source)
	}

	err = copyFile(steamAPI64Source, steamAPI64GamePath)
	if err != nil {
		return "", fmt.Errorf("failed to copy new steam_api64.dll: %v", err)
	}
	fmt.Printf("✅ Copied new steam_api64.dll to: %s\n", steamAPI64GamePath)

	return steamAPI64GamePath, nil
}

func updateConfigWithSteamID(appID string, steamID uint64) error {
	// Get current executable directory
	exePath, err := os.Executable()
	if err != nil {
		return fmt.Errorf("failed to get executable path: %v", err)
	}
	exeDir := filepath.Dir(exePath)

	// Config file path
	configFile := filepath.Join(exeDir, "utils", "steam_settings", fmt.Sprintf("configs.user.%s.ini", appID))

	// Read existing file
	content, err := os.ReadFile(configFile)
	if err != nil {
		return fmt.Errorf("failed to read config file %s: %v", configFile, err)
	}

	lines := strings.Split(string(content), "\n")

	// Update alt_steamid line
	altSteamIDUpdated := false

	for i, line := range lines {
		trimmed := strings.TrimSpace(line)

		// Update alt_steamid line
		if strings.HasPrefix(trimmed, "alt_steamid=") || strings.HasPrefix(trimmed, "# alt_steamid=") {
			lines[i] = fmt.Sprintf("alt_steamid=%d", steamID)
			altSteamIDUpdated = true
		}
	}

	// Write file
	updatedContent := strings.Join(lines, "\n")
	err = os.WriteFile(configFile, []byte(updatedContent), 0644)
	if err != nil {
		return fmt.Errorf("failed to update config file %s: %v", configFile, err)
	}

	fmt.Printf("✅ Updated config file: %s\n", configFile)
	if altSteamIDUpdated {

	} else {
		fmt.Println("⚠️ No alt_steamid line found in config")
	}

	return nil
}

func copySteamSettingsToGame(targetDir, appID string) error {
	// Get current executable directory
	exePath, err := os.Executable()
	if err != nil {
		return fmt.Errorf("failed to get executable path: %v", err)
	}
	exeDir := filepath.Dir(exePath)

	// Source directory
	steamSettingsDir := filepath.Join(exeDir, "utils", "steam_settings")

	// Check if steam_settings directory exists
	if _, err := os.Stat(steamSettingsDir); os.IsNotExist(err) {
		return fmt.Errorf("steam_settings directory not found: %s", steamSettingsDir)
	}

	copiedCount := 0
	currentAppConfigFile := fmt.Sprintf("configs.user.%s.ini", appID)

	// Use filepath.Walk to copy entire directory structure
	err = filepath.Walk(steamSettingsDir, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return nil // Continue even if there's an error with one file
		}

		// Skip the root steam_settings directory itself
		if path == steamSettingsDir {
			return nil
		}

		// Get relative path from steam_settings directory
		relPath, err := filepath.Rel(steamSettingsDir, path)
		if err != nil {
			return nil
		}

		fileName := info.Name()

		// Skip other configs.user files
		if strings.HasPrefix(fileName, "configs.user.") && strings.HasSuffix(fileName, ".ini") {
			if fileName == "configs.user.ini" {
				return nil
			}
			if fileName != currentAppConfigFile {
				return nil
			}
		}

		destPath := filepath.Join(targetDir, "steam_settings", relPath)

		if info.IsDir() {
			// Create directory
			if err := os.MkdirAll(destPath, info.Mode()); err != nil {
				return nil
			}
			return nil
		}

		// Handle files
		var finalDestPath string
		if fileName == currentAppConfigFile {
			// Rename configs.user.{AppID}.ini to configs.user.ini
			finalDestPath = filepath.Join(filepath.Dir(destPath), "configs.user.ini")
		} else {
			finalDestPath = destPath
		}

		// Ensure destination directory exists
		if err := os.MkdirAll(filepath.Dir(finalDestPath), 0755); err != nil {
			return nil
		}

		// Copy file
		err = copyFile(path, finalDestPath)
		if err != nil {
			return nil
		}

		copiedCount++
		return nil
	})

	if err != nil {
		return fmt.Errorf("failed to copy steam_settings: %v", err)
	}

	fmt.Printf("✅ Steam settings copied successfully\n")
	return nil
}
