package types

// ChunkBytes divides a byte slice into chunks of specified size
func ChunkBytes(data []byte, chunkSize int) [][]byte {
	chunksCount := (len(data) + chunkSize - 1) / chunkSize
	chunks := make([][]byte, chunksCount)

	for i := range chunks {
		start := i * chunkSize
		end := start + chunkSize
		if end > len(data) {
			end = len(data)
		}
		chunks[i] = data[start:end]
	}

	return chunks
}
