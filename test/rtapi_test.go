package test

import (
	"testing"

	"github.com/gorilla/websocket"
)

func _TestExample(t *testing.T) {
	t.Run("basic", func(t *testing.T) {
		// create a new WebSocket connection
		url := "ws://localhost:8080/ws"
		conn, _, err := websocket.DefaultDialer.Dial(url, nil)
		if err != nil {
			t.Fatalf("Failed to connect to WebSocket: %v", err)
		}
		defer conn.Close()
		// use goroutines to send and receive messages
		done := make(chan struct{})
		go func() {
			defer close(done)
			for {
				_, message, err := conn.ReadMessage()
				if err != nil {
					t.Errorf("Error reading message: %v", err)
					return
				}
				t.Logf("Received: %s", message)
			}
		}()
		go func() {
			defer close(done)
			for i := 0; i < 5; i++ {
				err := conn.WriteMessage(websocket.TextMessage, []byte("Hello, World!"))
				if err != nil {
					t.Errorf("Error writing message: %v", err)
					return
				}
			}
		}()
	})
}
