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
	"os"
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
	fmt.Println("1. Extract")
	fmt.Print("Enter your choice (1): ")

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
	configFile := filepath.Join(filepath.Dir(exeDir), "utils", "steam_settings", "configs.user.ini")

	// Read existing file
	content, err := os.ReadFile(configFile)
	if err != nil {
		fmt.Printf("❌ Failed to read config file %s: %v\n", configFile, err)
		return
	}

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
