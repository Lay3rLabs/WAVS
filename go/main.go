// main.go
package main

import (
	"bufio"
	_ "embed"
	"fmt"
	"io"
	"log"
	"os"
	"os/exec"
	"sync"
	"time"
)

//go:embed bin/wavs
var wavsBinary []byte

//go:embed wavs.toml
var wavsConfig []byte

// runEmbeddedBinary runs the embedded binary using temp files on all platforms
func runEmbeddedBinary() (*exec.Cmd, error) {
	// Create data directory
	dataDir := "./wavs-data"
	os.MkdirAll(dataDir, 0755)

	// Write binary to temp file
	tmpBinary, err := os.CreateTemp("", "wavs-*")
	if err != nil {
		return nil, fmt.Errorf("failed to create temp binary: %v", err)
	}

	if _, err := tmpBinary.Write(wavsBinary); err != nil {
		tmpBinary.Close()
		os.Remove(tmpBinary.Name())
		return nil, fmt.Errorf("failed to write binary: %v", err)
	}

	if err := tmpBinary.Chmod(0755); err != nil {
		tmpBinary.Close()
		os.Remove(tmpBinary.Name())
		return nil, fmt.Errorf("failed to chmod binary: %v", err)
	}

	binaryPath := tmpBinary.Name()
	tmpBinary.Close()

	// Write config to temp file
	tmpConfig, err := os.CreateTemp("", "wavs-*.toml")
	if err != nil {
		os.Remove(binaryPath)
		return nil, fmt.Errorf("failed to create temp config: %v", err)
	}

	if _, err := tmpConfig.Write(wavsConfig); err != nil {
		tmpConfig.Close()
		os.Remove(binaryPath)
		os.Remove(tmpConfig.Name())
		return nil, fmt.Errorf("failed to write config: %v", err)
	}

	configPath := tmpConfig.Name()
	tmpConfig.Close()

	// Schedule cleanup after process starts
	go func() {
		// Wait a moment for process to start
		time.Sleep(100 * time.Millisecond)
		os.Remove(binaryPath)
		os.Remove(configPath)
	}()

	cmd := exec.Command(binaryPath, "--home", configPath, "--data", dataDir)
	return cmd, nil
}

func main() {
	fmt.Println("Main application started.")

	// Run the embedded binary
	cmd, err := runEmbeddedBinary()
	if err != nil {
		log.Fatalf("Failed to prepare embedded binary: %v", err)
	}

	// Get pipes for stdout and stderr
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		log.Fatalf("Failed to get stdout pipe: %v", err)
	}

	stderr, err := cmd.StderrPipe()
	if err != nil {
		log.Fatalf("Failed to get stderr pipe: %v", err)
	}

	// Start the sidecar process
	err = cmd.Start()
	if err != nil {
		log.Fatalf("Failed to start sidecar: %v", err)
	}
	fmt.Printf("Sidecar started with PID: %d\n", cmd.Process.Pid)

	// Use a WaitGroup to ensure we read all output before proceeding
	var wg sync.WaitGroup
	wg.Add(2)

	// Function to read and display output from a reader
	readOutput := func(reader io.Reader, prefix string) {
		defer wg.Done()
		scanner := bufio.NewScanner(reader)
		for scanner.Scan() {
			fmt.Printf("[%s] %s\n", prefix, scanner.Text())
		}
		if err := scanner.Err(); err != nil {
			log.Printf("Error reading %s: %v", prefix, err)
		}
	}

	// Start goroutines to read stdout and stderr
	go readOutput(stdout, "WAVS")
	go readOutput(stderr, "WAVS-ERR")

	// Do some work in the main application while the sidecar runs
	fmt.Println("Main application doing its work...")

	// Wait for output readers to finish
	go func() {
		wg.Wait()
	}()

	// Wait for the sidecar to finish
	err = cmd.Wait()
	if err != nil {
		log.Printf("Sidecar exited with error: %v", err)
	} else {
		fmt.Println("Sidecar finished successfully.")
	}

	fmt.Println("Main application exiting.")
}
