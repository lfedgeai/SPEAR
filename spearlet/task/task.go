package task

import (
	"fmt"

	"slices"

	log "github.com/sirupsen/logrus"
)

type TaskConfig struct {
	// task name
	Name     string
	Image    string
	Cmd      string
	Args     []string
	WorkDir  string
	HostAddr string
}

// task type enum
type TaskType int

const (
	TaskTypeUnknown TaskType = iota
	TaskTypeDocker           // 1
	TaskTypeProcess          // 2
	TaskTypeDylib            // 3
	TaskTypeWasm             // 4
)

// task status enum
type TaskStatus int

const (
	TaskStatusRunning TaskStatus = iota
	TaskStatusInit
	TaskStatusStopped
)

const (
	maxDataSize = 4096 * 1024
)

// message type []bytes
type Message []byte

type TaskID string

type TaskVar int

const (
	TVTest TaskVar = iota
	TVOpenAIBaseURL
	TVOpenAIAPIKey
)

type Task interface {
	ID() TaskID
	// start task
	Start() error
	// stop task
	Stop() error
	// get task name
	Name() string
	// get task status
	Status() TaskStatus
	// get task result
	GetResult() *error
	// get communication channel
	CommChannels() (chan Message, chan Message, error)
	// wait for task to finish
	Wait() (int, error)
	// next request id
	NextRequestID() uint64
	// set task variable
	SetVar(key TaskVar, value interface{})
	// get task variable
	GetVar(key TaskVar) (interface{}, bool)
	// register a function called when task is finished
	RegisterOnFinish(fn func(Task))
}

// interface for taskruntime
type TaskRuntime interface {
	// create task
	CreateTask(cfg *TaskConfig) (Task, error)
	Start() error
	Stop() error
}

// implement TaskRuntimeDylib
type DylibTaskRuntime struct {
}

func (d *DylibTaskRuntime) CreateTask(cfg *TaskConfig) (Task, error) {
	return nil, fmt.Errorf("not implemented")
}

func (d *DylibTaskRuntime) Start() error {
	return nil
}

func (d *DylibTaskRuntime) Stop() error {
	return nil
}

// implement TaskRuntimeWasm
type WasmTaskRuntime struct {
}

func (w *WasmTaskRuntime) CreateTask(cfg *TaskConfig) (Task, error) {
	return nil, fmt.Errorf("not implemented")
}

func (w *WasmTaskRuntime) Start() error {
	return nil
}

func (w *WasmTaskRuntime) Stop() error {
	return nil
}

type TaskRuntimeConfig struct {
	Debug              bool
	Cleanup            bool
	StartServices      bool
	SupportedTaskTypes []TaskType
}

type TaskRuntimeCollection struct {
	// task runtimes
	TaskRuntimes map[TaskType]TaskRuntime
	// task runtime config
	TaskRuntimeConfig *TaskRuntimeConfig
}

func NewTaskRuntimeCollection(cfg *TaskRuntimeConfig) *TaskRuntimeCollection {
	res := &TaskRuntimeCollection{
		TaskRuntimes:      make(map[TaskType]TaskRuntime),
		TaskRuntimeConfig: cfg,
	}
	res.initTaskRuntimes(cfg)
	return res
}

// initialize task runtimes
func (c *TaskRuntimeCollection) initTaskRuntimes(cfg *TaskRuntimeConfig) {
	if len(cfg.SupportedTaskTypes) == 0 {
		panic("no supported task types")
	}
	for _, taskType := range cfg.SupportedTaskTypes {
		log.Infof("Initializing task runtime: %v", taskType)
		switch taskType {
		case TaskTypeDocker:
			rt, err := NewDockerTaskRuntime(cfg)
			if err != nil {
				log.Warn("Failed to init Docker runtime")
				continue
			}
			c.TaskRuntimes[TaskTypeDocker] = rt
		case TaskTypeProcess:
			c.TaskRuntimes[TaskTypeProcess] = NewProcessTaskRuntime()
		case TaskTypeDylib:
			c.TaskRuntimes[TaskTypeDylib] = &DylibTaskRuntime{}
		case TaskTypeWasm:
			c.TaskRuntimes[TaskTypeWasm] = &WasmTaskRuntime{}
		default:
			panic("invalid task type")
		}
	}
}

func (c *TaskRuntimeCollection) Cleanup() {
	for t, rt := range c.TaskRuntimes {
		log.Infof("Cleaning up task runtime type: %v", t)
		if err := rt.Stop(); err != nil {
			log.Errorf("Error stopping task runtime: %v", err)
		}
	}
}

func (c *TaskRuntimeCollection) GetTaskRuntime(taskType TaskType) (TaskRuntime, error) {
	if rt, ok := c.TaskRuntimes[taskType]; ok {
		return rt, nil
	}
	return nil, fmt.Errorf("task runtime not found")
}

// register task runtime
func (cfg *TaskRuntimeConfig) RegisterSupportedTaskType(taskType TaskType) {
	if slices.Contains(cfg.SupportedTaskTypes, taskType) {
		log.Warnf("Task type %v already registered", taskType)
		return
	}
	cfg.SupportedTaskTypes = append(cfg.SupportedTaskTypes, taskType)
	log.Infof("Registered task type %v", taskType)
}

// unregister task runtime
func (cfg *TaskRuntimeConfig) UnregisterSupportedTaskType(taskType TaskType) {
	for i, ty := range cfg.SupportedTaskTypes {
		if ty == taskType {
			cfg.SupportedTaskTypes = append(cfg.SupportedTaskTypes[:i], cfg.SupportedTaskTypes[i+1:]...)
			log.Infof("Unregistered task type %v", taskType)
			return
		}
	}
	log.Warnf("Task type %v not found", taskType)
}
