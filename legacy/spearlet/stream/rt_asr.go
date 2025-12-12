package stream

import (
	"encoding/base64"
	"encoding/json"
	"fmt"

	log "github.com/sirupsen/logrus"

	"github.com/gorilla/websocket"
	"github.com/lfedgeai/spear/pkg/spear/proto/stream"
	"github.com/lfedgeai/spear/spearlet/core"
	"github.com/lfedgeai/spear/spearlet/stream/backend"
	"github.com/lfedgeai/spear/spearlet/task"
)

const (
	NotificationEventCreated   = "rt-asr.created"
	NotificationEventError     = "rt-asr.error"
	NotificationEventDelta     = "rt-asr.delta"
	NotificationEventCompleted = "rt-asr.completed"
	NotificationEventStopped   = "rt-asr.stopped"
	NotificationEventAppended  = "rt-asr.appended"
)

type RtASRSession struct {
	TaskID   task.TaskID
	StreamID int32
	WSocket  *websocket.Conn
}

type rtASRStreamFunction struct {
	sessions map[task.TaskID]RtASRSession
}

func NewRtASRStreamFunction() core.StreamFunction {
	return &rtASRStreamFunction{
		sessions: make(map[task.TaskID]RtASRSession),
	}
}

func (r *rtASRStreamFunction) Name() string {
	return "rt-asr"
}

func (r *rtASRStreamFunction) Operation(sc core.StreamBiChannel,
	op stream.OperationType,
	data []byte, final bool) error {

	streamId := sc.StreamId()
	inv := sc.GetInvocationInfo()
	if inv == nil {
		return fmt.Errorf("invocation info is nil")
	}

	t := inv.Task
	if t == nil {
		return fmt.Errorf("task is nil for stream id %d", streamId)
	}
	taskId := t.ID()

	switch op {
	case stream.OperationTypeCreate:
		if _, exists := r.sessions[taskId]; exists {
			return fmt.Errorf("session already exists for task id %s", taskId)
		}
		s, err := backend.CreateRealtimeTranscriptionSession(backend.NewDefaultRealtimeTranscriptionSessionConfig())
		if err != nil {
			log.Fatalf("Failed to create realtime transcription session: %v", err)
		}

		c, err := backend.CreateRealtimeTranscriptionWebsocket(s.ClientSecret.Value)
		if err != nil {
			log.Fatalf("Failed to create websocket connection: %v", err)
		}
		r.sessions[taskId] = RtASRSession{
			TaskID:   taskId,
			StreamID: streamId,
			WSocket:  c,
		}

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
				backend.ProcessMessage(message, sc)
			}
		}()

		sc.WriteNotificationToTask("", stream.NotificationEventTypeCreated, []byte{}, false)
	case stream.OperationTypeAppend:
		session, exists := r.sessions[taskId]
		if !exists {
			log.Errorf("no session found for task id %s", taskId)
			sc.WriteNotificationToTask("", stream.NotificationEventTypeError,
				[]byte("no session found"), false)
			return fmt.Errorf("no session found for task id %s", taskId)
		}
		if session.WSocket == nil {
			log.Errorf("websocket connection is nil for task id %s", taskId)
			sc.WriteNotificationToTask("", stream.NotificationEventTypeError,
				[]byte("websocket connection is nil"), false)
			return fmt.Errorf("websocket connection is nil for task id %s", taskId)
		}
		audioBase64 := base64.StdEncoding.EncodeToString(data)
		event := backend.TranscriptionAppendBufferEvent{
			Type:  backend.MsgActionEventBufferAppend,
			Audio: audioBase64,
		}
		eventBytes, err := json.Marshal(event)
		if err != nil {
			log.Errorf("failed to marshal event: %v", err)
			sc.WriteNotificationToTask("", stream.NotificationEventTypeError,
				[]byte("failed to marshal event"), false)
			return fmt.Errorf("failed to marshal event: %v", err)
		}
		if err := session.WSocket.WriteMessage(websocket.TextMessage, eventBytes); err != nil {
			log.Errorf("failed to write message to websocket: %v", err)
			sc.WriteNotificationToTask("", stream.NotificationEventTypeError,
				[]byte("failed to write message to websocket"), false)
			return fmt.Errorf("failed to write message to websocket: %v", err)
		}
		// TODO: handle the response from the websocket
		sc.WriteNotificationToTask(NotificationEventAppended,
			stream.NotificationEventTypeUpdated,
			[]byte("audio data appended"), false)
	default:
		return fmt.Errorf("unsupported operation %s for stream id %d", op, streamId)
	}

	// sc.WriteNotificationToTask("op reply", stream.NotificationEventTypeCompleted,
	// 	[]byte("dummy"), false)
	return nil
}

func (r *rtASRStreamFunction) Notification(sc core.StreamBiChannel,
	op stream.NotificationEventType,
	data []byte, final bool) error {
	// sc.WriteNotificationToTask("notification reply", stream.NotificationEventTypeCompleted,
	// 	[]byte("dummy"), false)
	return fmt.Errorf("not implemented")
}

func (r *rtASRStreamFunction) Raw(sc core.StreamBiChannel,
	data []byte, final bool) error {
	return fmt.Errorf("not implemented")
}

var (
	rtASRStreamClass = core.NewStreamClass("rt-asr")
)

var messageHandlers = map[string]func(message []byte, priv interface{}) error{
	backend.MsgHandlerEventTransSessionCreated: func(message []byte, priv interface{}) error {
		// This function handles the "transcription_session.created" event.
		var event backend.TranscriptionSessionCreatedEvent
		err := json.Unmarshal(message, &event)
		if err != nil {
			log.Errorf("Failed to unmarshal message: %v", err)
			return err
		}
		log.Infof("Transcription session created: %+v", event.Session)
		return nil
	},
	backend.MsgHandlerEventTransSessionUpdated: func(message []byte, priv interface{}) error {
		log.Info("Received transcription session updated event")
		return nil
	},
	backend.MsgHandlerEventError: func(message []byte, priv interface{}) error {
		// This function handles the "error" event.
		var event backend.ErrorEvent
		err := json.Unmarshal(message, &event)
		if err != nil {
			log.Errorf("Failed to unmarshal error message: %v", err)
		}
		log.Errorf("Error event received: %s - %s", event.Error.Code, event.Error.Message)
		return nil
	},
	backend.MsgHandlerEventDefault: func(message []byte, priv interface{}) error {
		// This is a default handler for any message type that does not have a specific handler.
		log.Warnf("Received message of unknown type: %s", message)
		return nil
	},
	backend.MsgHandlerEventTransDelta: func(message []byte, priv interface{}) error {
		// This function handles the "conversation
		// .item.input_audio_transcription.delta" event.
		var event backend.TranscriptionDeltaEvent
		err := json.Unmarshal(message, &event)
		if err != nil {
			log.Errorf("Failed to unmarshal delta message: %v", err)
			return err
		}
		// convert priv to core.StreamBiChannel
		sc, ok := priv.(core.StreamBiChannel)
		if !ok {
			log.Errorf("Failed to convert priv to core.StreamBiChannel")
			return fmt.Errorf("priv is not a core.StreamBiChannel")
		}
		// Write the delta to the stream channel
		sc.WriteNotificationToTask(NotificationEventDelta, stream.NotificationEventTypeUpdated,
			[]byte(event.Delta), false)
		log.Infof("Transcription delta received: %s", event.Delta)
		return nil
	},
	backend.MsgHandlerEventTransCompleted: func(message []byte, priv interface{}) error {
		// This function handles the "conversation
		// .item.input_audio_transcription.completed" event.
		var event backend.TranscriptionCompletedEvent
		err := json.Unmarshal(message, &event)
		if err != nil {
			log.Errorf("Failed to unmarshal completed message: %v", err)
			return err
		}
		// convert priv to core.StreamBiChannel
		sc, ok := priv.(core.StreamBiChannel)
		if !ok {
			log.Errorf("Failed to convert priv to core.StreamBiChannel")
			return fmt.Errorf("priv is not a core.StreamBiChannel")
		}
		// Write the completed transcription to the stream channel
		sc.WriteNotificationToTask(NotificationEventCompleted, stream.NotificationEventTypeUpdated,
			[]byte(event.Transcript), false)
		log.Infof("Transcription completed: %s", event.Transcript)
		return nil
	},
	backend.MsgHandlerEventTransFailed: func(message []byte, priv interface{}) error {
		// This function handles the "conversation
		var event backend.TranscriptionFailedEvent
		err := json.Unmarshal(message, &event)
		if err != nil {
			log.Errorf("Failed to unmarshal failed message: %v", err)
			return err
		}
		log.Errorf("Transcription failed: %s - %s", event.Error.Code, event.Error.Message)
		return nil
	},
	backend.MsgHandlerEventIteamCreated: func(message []byte, priv interface{}) error {
		// This function handles the "conversation.item.created" event.
		var event backend.ConversationItemCreatedEvent
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
	backend.MsgHandlerEventCommitted: func(message []byte, priv interface{}) error {
		// This function handles the "input_audio_buffer.committed" event.
		var event backend.InputAudioBufferSpeechCommittedEvent
		err := json.Unmarshal(message, &event)
		if err != nil {
			log.Errorf("Failed to unmarshal speech committed message: %v", err)
			return err
		}
		log.Infof("Speech committed for item %s, previous item %s", event.ItemID, event.PreviousItemID)
		return nil
	},
	backend.MsgHandlerEventSpeechStarted: func(message []byte, priv interface{}) error {
		// This function handles the "input_audio_buffer.speech_started" event.
		var event backend.InputAudioBufferSpeechStartedEvent
		err := json.Unmarshal(message, &event)
		if err != nil {
			log.Errorf("Failed to unmarshal speech started message: %v", err)
			return err
		}
		log.Infof("Speech started at %d ms for item %s", event.AudioStartMs, event.ItemID)
		return nil
	},
	backend.MsgHandlerEventSpeechStopped: func(message []byte, priv interface{}) error {
		// This function handles the "input_audio_buffer.speech_stopped" event.
		var event backend.InputAudioBufferSpeechStoppedEvent
		err := json.Unmarshal(message, &event)
		if err != nil {
			log.Errorf("Failed to unmarshal speech stopped message: %v", err)
			return err
		}
		log.Infof("Speech stopped at %d ms for item %s", event.AudioEndMs, event.ItemID)
		// convert priv to core.StreamBiChannel
		sc, ok := priv.(core.StreamBiChannel)
		if !ok {
			log.Errorf("Failed to convert priv to core.StreamBiChannel")
			return fmt.Errorf("priv is not a core.StreamBiChannel")
		}
		// Write the speech stopped event to the stream channel
		sc.WriteNotificationToTask(NotificationEventStopped, stream.NotificationEventTypeUpdated,
			[]byte{}, false)
		return nil
	},
}

func init() {
	core.RegisterStreamClass(rtASRStreamClass)
	backend.SetMessageHandlers(messageHandlers)
	if err := rtASRStreamClass.RegisterStreamFunction(NewRtASRStreamFunction()); err != nil {
		panic(err)
	}
}
