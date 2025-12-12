package backend

import (
	"bytes"
	"encoding/binary"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"time"

	"github.com/gorilla/websocket"
	log "github.com/sirupsen/logrus"
)

const (
	// DefaultOpenAIAPIKey is the default API key for OpenAI.
	DefaultOpenAIAPIKey = "invalid_key"
)

const (
	MsgHandlerEventDefault             = "default"
	MsgHandlerEventError               = "error"
	MsgHandlerEventTransSessionCreated = "transcription_session.created"
	MsgHandlerEventTransSessionUpdated = "transcription_session.updated"
	MsgHandlerEventTransDelta          = "conversation.item.input_audio_transcription.delta"
	MsgHandlerEventTransCompleted      = "conversation.item.input_audio_transcription.completed"
	MsgHandlerEventTransFailed         = "conversation.item.input_audio_transcription.failed"
	MsgHandlerEventIteamCreated        = "conversation.item.created"
	MsgHandlerEventSpeechStarted       = "input_audio_buffer.speech_started"
	MsgHandlerEventSpeechStopped       = "input_audio_buffer.speech_stopped"
	MsgHandlerEventCommitted           = "input_audio_buffer.committed"
)

const (
	MsgActionEventBufferAppend = "input_audio_buffer.append"
)

// config for client secret expiration
type ExpiresAt struct {
	Anchor  string `json:"anchor,omitempty"`
	Seconds int    `json:"seconds,omitempty"`
}

// config for client secret\
type ClientSecretConfig struct {
	ExpiresAt *ExpiresAt `json:"expires_at,omitempty"`
}

// config for input noise reduction
type InputAudioNoiseReduction struct {
	Type string `json:"type,omitempty"`
}

type TurnDetectionConfig struct {
	CreateResponse    bool    `json:"create_response,omitempty"`
	Eagerness         string  `json:"eagerness,omitempty"`
	InterruptResponse bool    `json:"interrupt_response,omitempty"`
	PrefixPaddingMs   int     `json:"prefix_padding_ms,omitempty"`
	SilenceDurationMs int     `json:"silence_duration_ms,omitempty"`
	Type              string  `json:"type,omitempty"`
	Threshold         float64 `json:"threshold,omitempty"`
}

// config to create/update realtime transcription session
type RealtimeTranscriptionSessionConfig struct {
	ClientSecret             *ClientSecretConfig       `json:"client_secret,omitempty"`
	InputAudioFormat         string                    `json:"input_audio_format,omitempty"`
	InputAudioNoiseReduction *InputAudioNoiseReduction `json:"input_audio_noise_reduction,omitempty"`
	InputAudioTranscription  *InputAudioTranscription  `json:"input_audio_transcription,omitempty"`
	Modalities               []string                  `json:"modalities,omitempty"`
	TurnDetection            *TurnDetectionConfig      `json:"turn_detection,omitempty"`
	Include                  string                    `json:"include,omitempty"`
}

// event to update a realtime transcription session
type TranscriptionSessionUpdateEvent struct {
	Type    string                             `json:"type"`
	Session RealtimeTranscriptionSessionConfig `json:"session"`
}

// Create a transcription session config using default values
func NewDefaultRealtimeTranscriptionSessionConfig() RealtimeTranscriptionSessionConfig {
	return RealtimeTranscriptionSessionConfig{
		InputAudioFormat: "pcm16",
		InputAudioTranscription: &InputAudioTranscription{
			// setting the default model to gpt-4o-mini-transcribe for now
			Model: "gpt-4o-mini-transcribe",
		},
	}
}

type RealtimeTranscriptionSession struct {
	ID                      string                  `json:"id"`
	Object                  string                  `json:"object"`
	ExpiresAt               int64                   `json:"expires_at"`
	Modalities              []string                `json:"modalities"`
	TurnDetection           TurnDetection           `json:"turn_detection"`
	InputAudioFormat        string                  `json:"input_audio_format"`
	InputAudioTranscription InputAudioTranscription `json:"input_audio_transcription"`
	ClientSecret            ClientSecret            `json:"client_secret"`
	Include                 *string                 `json:"include,omitempty"`
}

type TurnDetection struct {
	Type              string  `json:"type"`
	Threshold         float64 `json:"threshold"`
	PrefixPaddingMs   int     `json:"prefix_padding_ms"`
	SilenceDurationMs int     `json:"silence_duration_ms"`
}

type InputAudioTranscription struct {
	Model    string  `json:"model,omitempty"`
	Language *string `json:"language,omitempty"`
	Prompt   string  `json:"prompt,omitempty"`
}

type ClientSecret struct {
	Value     string `json:"value"`
	ExpiresAt int64  `json:"expires_at"`
}

func CreateRealtimeTranscriptionSession(cfg RealtimeTranscriptionSessionConfig) (*RealtimeTranscriptionSession, error) {
	cfgJson, err := json.Marshal(cfg)
	if err != nil {
		log.Errorf("Failed to marshal config: %v", err)
		return nil, err
	}

	log.Info("Creating a new realtime transcription session...")
	// Send post request to OpenAI API
	client := &http.Client{Timeout: 10 * time.Second}
	req, err := http.NewRequest("POST",
		"https://api.openai.com/v1/realtime/transcription_sessions",
		bytes.NewBuffer(cfgJson))
	if err != nil {
		log.Errorf("Failed to create request: %v", err)
		return nil, err
	}

	apiKey := os.Getenv("OPENAI_API_KEY")
	if apiKey == "" {
		log.Warn("OPENAI_API_KEY is not set, using default API key")
		apiKey = DefaultOpenAIAPIKey
	}

	req.Header.Set("Authorization", fmt.Sprintf("Bearer %s", apiKey))
	req.Header.Set("Content-Type", "application/json")

	resp, err := client.Do(req)
	if err != nil {
		log.Errorf("Failed to send request: %v", err)
		return nil, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		log.Errorf("Unexpected status code: %d", resp.StatusCode)
		return nil, fmt.Errorf("unexpected status code: %d", resp.StatusCode)
	}

	var body []byte
	body, err = io.ReadAll(resp.Body)
	if err != nil {
		log.Errorf("Failed to read response body: %v", err)
		return nil, err
	}
	log.Infof("Response body: %s", body)

	// deserialize the response string into a RealtimeTranscriptionSession struct
	var session RealtimeTranscriptionSession
	err = json.Unmarshal(body, &session)
	if err != nil {
		log.Errorf("Failed to unmarshal response: %v", err)
		return nil, err
	}
	log.Infof("Created realtime transcription session: %+v", session)
	return &session, nil
}

func CreateRealtimeTranscriptionWebsocket(secret string) (*websocket.Conn, error) {
	headers := make(http.Header)
	headers.Set("Authorization", fmt.Sprintf("Bearer %s", secret))
	headers.Set("OpenAI-Beta", "realtime=v1")

	u := url.URL{Scheme: "wss", Host: "api.openai.com", Path: "/v1/realtime",
		RawQuery: "intent=transcription"}
	fmt.Printf("connecting to %s\n", u.String())
	c, _, err := websocket.DefaultDialer.Dial(u.String(), headers)
	if err != nil {
		log.Fatal("dial:", err)
	}
	return c, err
}

type TranscriptionSessionCreatedEvent struct {
	Type    string                       `json:"type"`
	EventID string                       `json:"event_id"`
	Session RealtimeTranscriptionSession `json:"session"`
}

type ErrorEvent struct {
	Type    string `json:"type"`
	EventID string `json:"event_id"`
	Error   struct {
		Type    string  `json:"type"`
		Code    string  `json:"code"`
		Message string  `json:"message"`
		Param   any     `json:"param,omitempty"`
		EventID *string `json:"event_id,omitempty"`
	} `json:"error"`
}

type TranscriptionDeltaEvent struct {
	Type         string `json:"type"`
	EventID      string `json:"event_id"`
	ItemID       string `json:"item_id"`
	ContentIndex int    `json:"content_index"`
	Delta        string `json:"delta"`
}

type TranscriptionCompletedEvent struct {
	Type         string `json:"type"`
	EventID      string `json:"event_id"`
	ItemID       string `json:"item_id"`
	ContentIndex int    `json:"content_index"`
	Transcript   string `json:"transcript"`
}

type TranscriptionFailedEvent struct {
	Type         string `json:"type"`
	EventID      string `json:"event_id"`
	ItemID       string `json:"item_id"`
	ContentIndex int    `json:"content_index"`
	Error        struct {
		Type    string `json:"type"`
		Code    string `json:"code"`
		Message string `json:"message"`
		Param   any    `json:"param,omitempty"`
	} `json:"error"`
}

// ConversationItemCreatedEvent represents an event when a new item is created in the conversation.
//
//	{
//	    "event_id": "event_1920",
//	    "type": "conversation.item.created",
//	    "previous_item_id": "msg_002",
//	    "item": {
//	        "id": "msg_003",
//	        "object": "realtime.item",
//	        "type": "message",
//	        "status": "completed",
//	        "role": "user",
//	        "content": []
//	    }
//	}
type ConversationItemCreatedEvent struct {
	EventID        string `json:"event_id"`         // Unique identifier for the event
	Type           string `json:"type"`             // Type of the event, e.g., "conversation.item.created"
	PreviousItemID string `json:"previous_item_id"` // Identifier for the previous item in
	// the conversation
	Item struct {
		ID      string `json:"id"`      // Unique identifier for the item
		Object  string `json:"object"`  // Type of the object, e.g., "realtime.item"
		Type    string `json:"type"`    // Type of the item, e.g., "message"
		Status  string `json:"status"`  // Status of the item, e.g, "completed"
		Role    string `json:"role"`    // Role of the item, e.g., "user"
		Content []any  `json:"content"` // Content of the item, which can be an array of various types
	} `json:"item"`
}

// TranscriptionAppendBufferEvent represents an event to append audio data to the transcription buffer.
//
//	{
//	    "event_id": "event_456",
//	    "type": "input_audio_buffer.append",
//	    "audio": "Base64EncodedAudioData"
//	}
type TranscriptionAppendBufferEvent struct {
	EventID string `json:"event_id,omitempty"`
	Type    string `json:"type"`
	Audio   string `json:"audio"` // Base64 encoded audio data
}

// InputAudioBufferSpeechCommittedEvent represents an event indicating that speech has been committed in the audio buffer.
// An example message might look like this:
//
//	{
//	    "event_id": "event_1121",
//	    "type": "input_audio_buffer.committed",
//	    "previous_item_id": "msg_001",
//	    "item_id": "msg_002"
//	}
type InputAudioBufferSpeechCommittedEvent struct {
	EventID        string `json:"event_id"`         // Unique identifier for the event
	Type           string `json:"type"`             // Type of the event, e.g., "input_audio_buffer.committed"
	PreviousItemID string `json:"previous_item_id"` // Identifier for the previous item in the conversation
	ItemID         string `json:"item_id"`          // Identifier for the current item in the conversation
}

// InputAudioBufferSpeechStartedEvent represents an event indicating that speech has started in the audio buffer.
// An example message might look like this:
//
//	{
//	    "event_id": "event_1516",
//	    "type": "input_audio_buffer.speech_started",
//	    "audio_start_ms": 1000,
//	    "item_id": "msg_003"
//	}
type InputAudioBufferSpeechStartedEvent struct {
	EventID      string `json:"event_id"`
	Type         string `json:"type"`
	AudioStartMs int    `json:"audio_start_ms"` // Timestamp in milliseconds when speech started
	ItemID       string `json:"item_id"`        // Identifier for the item in the conversation
}

// InputAudioBufferSpeechStoppedEvent represents an event indicating that speech has stopped in the audio buffer.
// An example message might look like this:
//
//	{
//	    "event_id": "event_1718",
//	    "type": "input_audio_buffer.speech_stopped",
//	    "audio_end_ms": 2000,
//	    "item_id": "msg_003"
//	}
type InputAudioBufferSpeechStoppedEvent struct {
	EventID    string `json:"event_id"`     // Unique identifier for the event
	Type       string `json:"type"`         // Type of the event, e.g., "input_audio_buffer.speech_stopped"
	AudioEndMs int    `json:"audio_end_ms"` // Timestamp in milliseconds when speech stopped
	ItemID     string `json:"item_id"`      // Identifier for the item in the conversation
}

var messageHandlers = map[string]func(message []byte, priv interface{}) error{
	MsgHandlerEventTransSessionCreated: func(message []byte, priv interface{}) error {
		// This function handles the "transcription_session.created" event.
		var event TranscriptionSessionCreatedEvent
		err := json.Unmarshal(message, &event)
		if err != nil {
			log.Errorf("Failed to unmarshal message: %v", err)
			return err
		}
		log.Infof("Transcription session created: %+v", event.Session)
		return nil
	},
	MsgHandlerEventTransSessionUpdated: func(message []byte, priv interface{}) error {
		log.Info("Received transcription session updated event")
		return nil
	},
	MsgHandlerEventError: func(message []byte, priv interface{}) error {
		// This function handles the "error" event.
		var event ErrorEvent
		err := json.Unmarshal(message, &event)
		if err != nil {
			log.Errorf("Failed to unmarshal error message: %v", err)
		}
		log.Errorf("Error event received: %s - %s", event.Error.Code, event.Error.Message)
		return nil
	},
	MsgHandlerEventDefault: func(message []byte, priv interface{}) error {
		// This is a default handler for any message type that does not have a specific handler.
		log.Warnf("Received message of unknown type: %s", message)
		return nil
	},
	MsgHandlerEventTransDelta: func(message []byte, priv interface{}) error {
		// This function handles the "conversation
		// .item.input_audio_transcription.delta" event.
		var event TranscriptionDeltaEvent
		err := json.Unmarshal(message, &event)
		if err != nil {
			log.Errorf("Failed to unmarshal delta message: %v", err)
			return err
		}
		log.Infof("Transcription delta received: %s", event.Delta)
		return nil
	},
	MsgHandlerEventTransCompleted: func(message []byte, priv interface{}) error {
		// This function handles the "conversation
		// .item.input_audio_transcription.completed" event.
		var event TranscriptionCompletedEvent
		err := json.Unmarshal(message, &event)
		if err != nil {
			log.Errorf("Failed to unmarshal completed message: %v", err)
			return err
		}
		log.Infof("Transcription completed: %s", event.Transcript)
		return nil
	},
	MsgHandlerEventTransFailed: func(message []byte, priv interface{}) error {
		// This function handles the "conversation
		var event TranscriptionFailedEvent
		err := json.Unmarshal(message, &event)
		if err != nil {
			log.Errorf("Failed to unmarshal failed message: %v", err)
			return err
		}
		log.Errorf("Transcription failed: %s - %s", event.Error.Code, event.Error.Message)
		return nil
	},
	MsgHandlerEventIteamCreated: func(message []byte, priv interface{}) error {
		// This function handles the "conversation.item.created" event.
		var event ConversationItemCreatedEvent
		err := json.Unmarshal(message, &event)
		if err != nil {
			log.Errorf("Failed to unmarshal item created message: %v", err)
			return err
		}
		log.Infof("Conversation item created: ID=%s, Role=%s, Status=%s",
			event.Item.ID, event.Item.Role, event.Item.Status)
		if len(event.Item.Content) > 0 {
			log.Infof("Item content: %v", event.Item.Content)
		} else {
			log.Info("Item content is empty")
		}
		return nil
	},
	MsgHandlerEventCommitted: func(message []byte, priv interface{}) error {
		// This function handles the "input_audio_buffer.committed" event.
		var event InputAudioBufferSpeechCommittedEvent
		err := json.Unmarshal(message, &event)
		if err != nil {
			log.Errorf("Failed to unmarshal speech committed message: %v", err)
			return err
		}
		log.Infof("Speech committed for item %s, previous item %s", event.ItemID, event.PreviousItemID)
		return nil
	},
	MsgHandlerEventSpeechStarted: func(message []byte, priv interface{}) error {
		// This function handles the "input_audio_buffer.speech_started" event.
		var event InputAudioBufferSpeechStartedEvent
		err := json.Unmarshal(message, &event)
		if err != nil {
			log.Errorf("Failed to unmarshal speech started message: %v", err)
			return err
		}
		log.Infof("Speech started at %d ms for item %s", event.AudioStartMs, event.ItemID)
		return nil
	},
	MsgHandlerEventSpeechStopped: func(message []byte, priv interface{}) error {
		// This function handles the "input_audio_buffer.speech_stopped" event.
		var event InputAudioBufferSpeechStoppedEvent
		err := json.Unmarshal(message, &event)
		if err != nil {
			log.Errorf("Failed to unmarshal speech stopped message: %v", err)
			return err
		}
		log.Infof("Speech stopped at %d ms for item %s", event.AudioEndMs, event.ItemID)
		return nil
	},
}

func SetMessageHandlers(handlers map[string]func(message []byte, priv interface{}) error) {
	// This function sets the message handlers for different event types.
	// It allows you to add or override handlers for specific event types.
	for eventType, handler := range handlers {
		if _, exists := messageHandlers[eventType]; exists {
			log.Warnf("Handler for event type %s already exists, overriding it", eventType)
		}
		messageHandlers[eventType] = handler
	}
	log.Infof("Message handlers set: %v", messageHandlers)
}

func ProcessMessage(message []byte, priv interface{}) {
	// unmarshal the message to get the event type string
	var event map[string]any
	err := json.Unmarshal(message, &event)
	if err != nil {
		log.Errorf("Failed to unmarshal message: %v", err)
		return
	}
	eventType, ok := event["type"].(string)
	if !ok {
		log.Error("Message does not contain a valid event type")
		return
	}
	handler, exists := messageHandlers[eventType]
	if !exists {
		// default handler
		handler = messageHandlers[MsgHandlerEventDefault]
		log.Warnf("No handler found for event type %s, using default handler", eventType)
	}
	err = handler(message, priv)
	if err != nil {
		log.Errorf("Failed to process message of type %s: %v", eventType, err)
		return
	}
}

func int16ToBytes(samples []int16) []byte {
	buf := make([]byte, len(samples)*2)                      // 2 bytes per int16
	binary.LittleEndian.PutUint16(buf, (uint16)(samples[0])) // just to show the usage
	// The idiomatic way:
	for i, v := range samples {
		binary.LittleEndian.PutUint16(buf[i*2:], uint16(v))
	}
	return buf
}
