package backend

import (
	"encoding/json"
	"fmt"
	"os"
	"os/signal"
	"testing"
	"time"

	"encoding/base64"

	"github.com/gordonklaus/portaudio"

	"github.com/gorilla/websocket"
	log "github.com/sirupsen/logrus"
)

func TestRTASR(t *testing.T) {
	interrupt := make(chan os.Signal, 1)
	signal.Notify(interrupt, os.Interrupt)

	s, err := CreateRealtimeTranscriptionSession(NewDefaultRealtimeTranscriptionSessionConfig())
	if err != nil {
		log.Fatalf("Failed to create realtime transcription session: %v", err)
	}

	c, err := CreateRealtimeTranscriptionWebsocket(s.ClientSecret.Value)
	if err != nil {
		log.Fatalf("Failed to create websocket connection: %v", err)
	}
	defer c.Close()

	done := make(chan struct{})

	go func() {
		defer close(done)
		for {
			_, message, err := c.ReadMessage()
			if err != nil {
				log.Println("read:", err)
				return
			}
			fmt.Printf("recv: %s\n", message)
			ProcessMessage(message, nil)
		}
	}()

	portaudio.Initialize()
	defer portaudio.Terminate()

	data := make([]int16, 10240) // Buffer for audio data
	stream, err := portaudio.OpenDefaultStream(1, 0, 24000, len(data), data)
	if err != nil {
		log.Fatalf("Failed to open audio stream: %v", err)
	}
	defer stream.Close()
	// Start the audio stream
	err = stream.Start()
	if err != nil {
		log.Fatalf("Failed to start audio stream: %v", err)
	}

	defer stream.Stop()

	dataCh := make(chan *[]int16, 10) // Channel to send audio data
	go func() {
		// Read audio data from the stream and put into a channel
		for {
			err := stream.Read()
			if err != nil {
				log.Fatalf("Failed to read audio data: %v", err)
			}
			dataCh <- &data
		}
	}()

	for {
		select {
		case <-done:
			return
		case audioData := <-dataCh:
			// Encode audio data to Base64
			audioBase64 := base64.StdEncoding.EncodeToString(int16ToBytes(*audioData))
			// Create an event to append audio data to the transcription buffer
			event := TranscriptionAppendBufferEvent{
				Type:  MsgActionEventBufferAppend,
				Audio: audioBase64,
			}
			eventBytes, err := json.Marshal(event)
			if err != nil {
				log.Printf("Failed to marshal append buffer event: %v", err)
				continue
			}
			log.Printf("Sending append buffer event")
			// Send the event to the websocket
			err = c.WriteMessage(websocket.TextMessage, eventBytes)
			if err != nil {
				log.Printf("Failed to send append buffer event: %v", err)
				continue
			}
			// log.Printf("Sent append buffer event: %s", eventBytes)
		case <-interrupt:
			log.Println("interrupt")
			// Cleanly close the connection by sending a close message and then waiting for the server to close the connection.
			err := c.WriteMessage(websocket.CloseMessage,
				websocket.FormatCloseMessage(websocket.CloseNormalClosure, ""))
			if err != nil {
				log.Println("write close:", err)
				return
			}
			select {
			case <-done:
			case <-time.After(time.Second):
			}
			return
		}
	}
}
