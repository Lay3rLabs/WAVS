// main.go
package main

import (
	"bufio"
	"fmt"
	"io"
	"log"
	"os/exec"
	"sync"
)

// embed the file located at /Users/reece/.cargo/bin/wavs

// TODO: change this to be embeded or something with a virtual FS?
// - cp target/release/wavs ./go/bin
// - cp wavs.toml ./go

func main() {
	fmt.Println("Main application started.")

	// Create a new command to run the sidecar.
	// mkdir -p wavs-data
	cmd := exec.Command("./bin/wavs", "--home", "wavs.toml", "--data", "./wavs-data")

	// Set environment variable to use a local data directory
	// cmd.Env = append(os.Environ(), "WAVS_DATA_DIR=./wavs-data")

	// Get pipes for stdout and stderr
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		log.Fatalf("Failed to get stdout pipe: %v", err)
	}

	stderr, err := cmd.StderrPipe()
	if err != nil {
		log.Fatalf("Failed to get stderr pipe: %v", err)
	}

	// Start the sidecar process.
	// Use Start() to run it asynchronously, allowing the main app to continue.
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

	// Do some work in the main application while the sidecar runs.
	fmt.Println("Main application doing its work...")

	// Wait for output readers to finish
	go func() {
		wg.Wait()
	}()

	// Wait for the sidecar to finish.
	// This is important to ensure the sidecar completes its tasks
	// and to handle any potential errors during its execution.
	err = cmd.Wait()
	if err != nil {
		log.Printf("Sidecar exited with error: %v", err)
	} else {
		fmt.Println("Sidecar finished successfully.")
	}

	fmt.Println("Main application exiting.")
}
