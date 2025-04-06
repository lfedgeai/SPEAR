package spearlet

import (
	"context"
	"fmt"
	"io"
	"math/rand"
	"net/http"
	"os"
	"path/filepath"
	"strconv"
	"time"

	"github.com/gin-gonic/gin"
	flatbuffers "github.com/google/flatbuffers/go"
	log "github.com/sirupsen/logrus"

	"github.com/lfedgeai/spear/pkg/common"
	"github.com/lfedgeai/spear/pkg/spear/proto/custom"
	"github.com/lfedgeai/spear/pkg/spear/proto/transport"
	hc "github.com/lfedgeai/spear/spearlet/hostcalls"
	hostcalls "github.com/lfedgeai/spear/spearlet/hostcalls/common"
	"github.com/lfedgeai/spear/spearlet/task"
	_ "github.com/lfedgeai/spear/spearlet/tools"

	"github.com/docker/docker/client"
	"github.com/gorilla/mux"
	"github.com/gorilla/websocket"
)

var (
	logLevel = log.InfoLevel
)

type SpearletConfig struct {
	Addr string
	Port string

	// Search Path
	SearchPath []string

	// Debug
	Debug bool

	SpearAddr string

	// backend service
	StartBackendServices bool

	CertFile string
	KeyFile  string
}

type Spearlet struct {
	cfg *SpearletConfig
	mux *http.ServeMux
	srv *http.Server

	hc      *hostcalls.HostCalls
	commMgr *hostcalls.CommunicationManager
	mQueues map[task.Task]map[uint16]chan task.Message

	spearAddr string

	isSSL    bool
	certFile string
	keyFile  string

	streamUpgrader websocket.Upgrader

	rtCollection *task.TaskRuntimeCollection
}

type TaskMetaData struct {
	Id        int64
	Type      task.TaskType
	ImageName string
	ExecName  string
	Name      string
	InStream  bool
	OutStream bool
}

var (
	tmpMetaData = map[int64]TaskMetaData{
		3: {
			Id:        3,
			Type:      task.TaskTypeDocker,
			ImageName: "gen_image:latest",
			Name:      "gen_image",
			InStream:  false,
			OutStream: false,
		},
		4: {
			Id:        4,
			Type:      task.TaskTypeDocker,
			ImageName: "pychat:latest",
			Name:      "pychat",
			InStream:  false,
			OutStream: false,
		},
		5: {
			Id:        5,
			Type:      task.TaskTypeDocker,
			ImageName: "pytools:latest",
			Name:      "pytools",
			InStream:  false,
			OutStream: false,
		},
		6: {
			Id:        6,
			Type:      task.TaskTypeDocker,
			ImageName: "pyconversation:latest",
			Name:      "pyconversation",
			InStream:  false,
			OutStream: false,
		},
		7: {
			Id:        7,
			Type:      task.TaskTypeDocker,
			ImageName: "pydummy:latest",
			Name:      "pydummy",
			InStream:  false,
			OutStream: false,
		},
		8: {
			Id:        8,
			Type:      task.TaskTypeDocker,
			ImageName: "pytest-functionality:latest",
			Name:      "pytest-functionality",
			InStream:  false,
			OutStream: false,
		},
	}
)

// NewServeSpearletConfig creates a new SpearletConfig
func NewServeSpearletConfig(addr, port string, spath []string, debug bool,
	spearAddr string, certFile string, keyFile string,
	startBackendService bool) (*SpearletConfig, error) {
	if certFile != "" && keyFile == "" || certFile == "" && keyFile != "" {
		return nil, fmt.Errorf("both cert and key files must be provided")
	}
	return &SpearletConfig{
		Addr:                 addr,
		Port:                 port,
		SearchPath:           spath,
		Debug:                debug,
		SpearAddr:            spearAddr,
		StartBackendServices: startBackendService,
		CertFile:             certFile,
		KeyFile:              keyFile,
	}, nil
}

func NewExecSpearletConfig(debug bool, spearAddr string, spath []string,
	startBackendServices bool) *SpearletConfig {
	return &SpearletConfig{
		Addr:                 "",
		Port:                 "",
		SearchPath:           spath,
		Debug:                debug,
		SpearAddr:            spearAddr,
		StartBackendServices: startBackendServices,
	}
}

func NewSpearlet(cfg *SpearletConfig) *Spearlet {
	w := &Spearlet{
		cfg:       cfg,
		mux:       http.NewServeMux(),
		hc:        nil,
		commMgr:   hostcalls.NewCommunicationManager(),
		mQueues:   map[task.Task]map[uint16]chan task.Message{},
		spearAddr: cfg.SpearAddr,
		streamUpgrader: websocket.Upgrader{
			ReadBufferSize:  1024 * 4,
			WriteBufferSize: 1024 * 4,
			CheckOrigin: func(r *http.Request) bool {
				return true
			},
		},
	}
	if cfg.CertFile != "" && cfg.KeyFile != "" {
		w.isSSL = true
		w.certFile = cfg.CertFile
		w.keyFile = cfg.KeyFile
	}
	hc := hostcalls.NewHostCalls(w.commMgr)
	w.hc = hc
	return w
}

func (w *Spearlet) Initialize() {
	w.addRoutes()
	w.addHostCalls()
	w.initializeRuntimes()
	go w.hc.Run()
}

func (w *Spearlet) addHostCalls() {
	for _, hc := range hc.Hostcalls {
		w.hc.RegisterHostCall(hc)
	}
}

func (w *Spearlet) initializeRuntimes() {
	cfg := &task.TaskRuntimeConfig{
		Debug:         w.cfg.Debug,
		Cleanup:       true,
		StartServices: w.cfg.StartBackendServices,
	}
	task.RegisterSupportedTaskType(task.TaskTypeDocker)
	task.RegisterSupportedTaskType(task.TaskTypeProcess)
	w.rtCollection = task.NewTaskRuntimeCollection(cfg)
}

func funcId(req *http.Request) (int64, error) {
	vars := mux.Vars(req)
	if id, ok := vars["funcId"]; ok {
		// convert id to int64
		i, err := strconv.ParseInt(id, 10, 64)
		if err != nil {
			return -1, fmt.Errorf("error parsing funcId: %v", err)
		}
		return i, nil
	}

	// get request headers
	headers := req.Header
	// get the id from the headers
	id := headers.Get(HeaderFuncId)
	if id == "" {
		return -1, fmt.Errorf("missing %s header", HeaderFuncId)
	}

	// convert id to int64
	i, err := strconv.ParseInt(id, 10, 64)
	if err != nil {
		return -1, fmt.Errorf("error parsing %s header: %v",
			HeaderFuncId, err)
	}

	return i, nil
}

func funcName(req *http.Request) (string, error) {
	// get request headers
	headers := req.Header
	// get the name from the headers
	name := headers.Get(HeaderFuncName)
	if name == "" {
		return "", fmt.Errorf("missing %s header", HeaderFuncName)
	}

	return name, nil
}

func funcType(req *http.Request) (task.TaskType, error) {
	// get request headers
	headers := req.Header
	// get the runtime from the headers
	runtime := headers.Get(HeaderFuncType)
	if runtime == "" {
		return task.TaskTypeUnknown,
			fmt.Errorf("missing %s header", HeaderFuncType)
	}

	// convert runtime to int
	i, err := strconv.Atoi(runtime)
	if err != nil {
		return task.TaskTypeUnknown,
			fmt.Errorf("error parsing %s header: %v", HeaderFuncType, err)
	}

	switch i {
	case int(task.TaskTypeDocker):
		return task.TaskTypeDocker, nil
	case int(task.TaskTypeProcess):
		return task.TaskTypeProcess, nil
	case int(task.TaskTypeDylib):
		return task.TaskTypeDylib, nil
	case int(task.TaskTypeWasm):
		return task.TaskTypeWasm, nil
	default:
		return task.TaskTypeUnknown,
			fmt.Errorf("invalid %s header: %s", HeaderFuncType, runtime)
	}
}

func (w *Spearlet) CommunicationManager() *hostcalls.CommunicationManager {
	return w.commMgr
}

func (w *Spearlet) LookupTaskId(name string) (int64, error) {
	for _, v := range tmpMetaData {
		if v.Name == name {
			return v.Id, nil
		}
	}
	return -1, fmt.Errorf("error: task name not found: %s", name)
}

func (w *Spearlet) ListTasks() []string {
	var tasks []string
	for _, v := range tmpMetaData {
		tasks = append(tasks, v.Name)
	}
	return tasks
}

func (w *Spearlet) RunTask(funcId int64, funcName string, funcType task.TaskType,
	method string, data string, reqStream chan task.Message, respStream chan task.Message,
	sendTermOnRtn bool, waitInstance bool) (
	respData string, err error) {
	t, respData, err := w.ExecuteTask(funcId, funcName, funcType, method, data,
		reqStream, respStream)
	if err != nil {
		return "", err
	}
	if sendTermOnRtn {
		if err := w.commMgr.SendOutgoingRPCSignal(t, transport.SignalTerminate,
			[]byte{}); err != nil {
			return "", fmt.Errorf("error: %v", err)
		}
	}
	if waitInstance {
		if _, err := t.Wait(); err != nil {
			log.Warnf("Error waiting for task: %v", err)
		}
	}
	return respData, nil
}

func (w *Spearlet) metaDataToTaskCfg(meta TaskMetaData) *task.TaskConfig {
	randSrc := rand.NewSource(time.Now().UnixNano())
	randGen := rand.New(randSrc)
	name := fmt.Sprintf("task-%s-%d", meta.Name, randGen.Intn(10000))
	switch meta.Type {
	case task.TaskTypeDocker:
		return &task.TaskConfig{
			Name:     name,
			Cmd:      "/start",
			Args:     []string{},
			Image:    meta.ImageName,
			WorkDir:  "",
			HostAddr: w.spearAddr,
		}
	case task.TaskTypeProcess:
		// go though search patch to find ExecName
		execName := ""
		execPath := ""
		for _, path := range w.cfg.SearchPath {
			log.Infof("Searching for exec %s in path %s", meta.ExecName, path)
			if _, err := os.Stat(filepath.Join(path, meta.ExecName)); err == nil {
				execName = filepath.Join(path, meta.ExecName)
				execPath = path
				break
			}
		}
		if execName == "" || execPath == "" {
			log.Errorf("Error: exec name %s and path %s not found",
				meta.ExecName, execPath)
			return nil
		}
		log.Infof("Using exec: %s", execName)
		return &task.TaskConfig{
			Name:     name,
			Cmd:      execName,
			Args:     []string{},
			Image:    "",
			WorkDir:  execPath,
			HostAddr: w.spearAddr,
		}
	default:
		return nil
	}
}

func (w *Spearlet) ExecuteTaskByName(taskName string, funcType task.TaskType, method string,
	reqData string, reqStream chan task.Message, respStream chan task.Message) (t task.Task,
	respData string, err error) {
	meta := TaskMetaData{
		Id: -1,
	}

	if _, err := w.rtCollection.GetTaskRuntime(funcType); err != nil {
		return nil, "", fmt.Errorf("error: task runtime not found: %d",
			funcType)
	}

	for _, v := range tmpMetaData {
		if v.Name == taskName {
			meta = v
			break
		}
	}

	if meta.Id == -1 {
		switch funcType {
		case task.TaskTypeDocker:
			// search if the docker image exists
			// if not, return error
			cli, err := client.NewClientWithOpts(client.FromEnv)
			if err != nil {
				return nil, "", fmt.Errorf("error: %v", err)
			}

			_, _, err = cli.ImageInspectWithRaw(context.Background(), taskName)
			if err != nil {
				return nil, "", fmt.Errorf("error: %v", err)
			}

			meta = TaskMetaData{
				Id:        -1,
				Type:      task.TaskTypeDocker,
				ImageName: taskName,
				Name:      taskName,
				InStream:  false,
				OutStream: false,
			}
		case task.TaskTypeProcess:
			meta = TaskMetaData{
				Id:        -1,
				Type:      task.TaskTypeProcess,
				ExecName:  taskName,
				Name:      taskName,
				InStream:  false,
				OutStream: false,
			}
		case task.TaskTypeDylib:
			panic("not implemented")
		case task.TaskTypeWasm:
			panic("not implemented")
		default:
			panic("invalid task type")
		}

		if reqStream != nil {
			meta.InStream = true
		}
		if respStream != nil {
			meta.OutStream = true
		}
	}

	log.Infof("Using metadata: %+v", meta)

	cfg := w.metaDataToTaskCfg(meta)
	if cfg == nil {
		return nil, "", fmt.Errorf("error: invalid task type: %d",
			funcType)
	}

	rt, err := w.rtCollection.GetTaskRuntime(funcType)
	if err != nil {
		return nil, "", fmt.Errorf("error: %v", err)
	}

	newTask, err := rt.CreateTask(cfg)
	if err != nil {
		return nil, "", fmt.Errorf("error: %v", err)
	}
	err = w.commMgr.InstallToTask(newTask)
	if err != nil {
		return nil, "", fmt.Errorf("error: %v", err)
	}

	log.Debugf("Starting task: %s", newTask.Name())
	newTask.Start()

	reqMQueueID := uint16(0)
	w.mQueues[newTask] = map[uint16]chan task.Message{
		reqMQueueID: make(chan task.Message, 1024),
	}
	respMQueueID := uint16(1)
	w.mQueues[newTask] = map[uint16]chan task.Message{
		respMQueueID: make(chan task.Message, 1024),
	}

	builder := flatbuffers.NewBuilder(512)
	methodOff := builder.CreateString(method)

	if reqStream == nil && respStream == nil {
		// no stream
		dataOff := builder.CreateString(reqData)

		custom.NormalRequestInfoStart(builder)
		custom.NormalRequestInfoAddParamsStr(builder, dataOff)
		infoOff := custom.NormalRequestInfoEnd(builder)

		custom.CustomRequestStart(builder)
		custom.CustomRequestAddMethodStr(builder, methodOff)
		custom.CustomRequestAddRequestInfoType(builder,
			custom.RequestInfoNormalRequestInfo)
		custom.CustomRequestAddRequestInfo(builder, infoOff)
		builder.Finish(custom.CustomRequestEnd(builder))
	} else {
		custom.StreamRequestInfoStart(builder)
		custom.StreamRequestInfoAddInQueueId(builder, int32(reqMQueueID))
		custom.StreamRequestInfoAddOutQueueId(builder, int32(respMQueueID))
		infoOff := custom.StreamRequestInfoEnd(builder)

		custom.CustomRequestStart(builder)
		custom.CustomRequestAddMethodStr(builder, methodOff)
		custom.CustomRequestAddRequestInfoType(builder,
			custom.RequestInfoStreamRequestInfo)
		custom.CustomRequestAddRequestInfo(builder, infoOff)
		builder.Finish(custom.CustomRequestEnd(builder))
	}

	if reqStream != nil {
		go func() {
			for msg := range reqStream {
				w.mQueues[newTask][reqMQueueID] <- msg
			}
		}()
	}

	if r, err := w.commMgr.SendOutgoingRPCRequest(newTask,
		transport.MethodCustom,
		builder.FinishedBytes()); err != nil {
		return nil, "", fmt.Errorf("error: %v", err)
	} else {
		if len(r.ResponseBytes()) == 0 {
			return newTask, "", nil
		}
		customResp := custom.GetRootAsCustomResponse(r.ResponseBytes(), 0)

		if customResp.ReturnStream() {
			// stream return
			queueId := respMQueueID
			// streaming response
			if _, ok := w.mQueues[newTask][uint16(queueId)]; !ok {
				return nil, "", fmt.Errorf("error: queue not found: %d",
					queueId)
			}

			go func() {
				for msg := range w.mQueues[newTask][uint16(queueId)] {
					respStream <- msg
				}
			}()

			return newTask, "", nil
		} else {
			customRespData := customResp.DataBytes()
			return newTask, string(customRespData), nil
		}
	}
}

func (w *Spearlet) ExecuteTaskById(taskId int64, funcType task.TaskType, method string,
	reqData string, reqStream chan task.Message, respStream chan task.Message) (t task.Task,
	respData string,
	err error) {
	// get metadata from taskId
	meta, ok := tmpMetaData[taskId]
	if !ok {
		return nil, "", fmt.Errorf("error: invalid task id: %d",
			taskId)
	}
	if funcType == task.TaskTypeUnknown {
		funcType = meta.Type
	}
	if meta.Type != funcType {
		return nil, "", fmt.Errorf("error: invalid task type: %d, %+v",
			funcType, meta)
	}
	if meta.InStream != (reqStream != nil) {
		return nil, "", fmt.Errorf("error: invalid task input stream: %v, %v",
			meta.InStream, reqStream != nil)
	}
	if meta.OutStream != (respStream != nil) {
		return nil, "", fmt.Errorf("error: invalid task output stream: %v, %v",
			meta.OutStream, respStream != nil)
	}

	log.Infof("Using metadata: %+v", meta)

	cfg := w.metaDataToTaskCfg(meta)
	if cfg == nil {
		return nil, "", fmt.Errorf("error: invalid task type: %d",
			funcType)
	}

	rt, err := w.rtCollection.GetTaskRuntime(funcType)
	if err != nil {
		return nil, "", fmt.Errorf("error: %v", err)
	}

	newTask, err := rt.CreateTask(cfg)
	if err != nil {
		return nil, "", fmt.Errorf("error: %v", err)
	}
	err = w.commMgr.InstallToTask(newTask)
	if err != nil {
		return nil, "", fmt.Errorf("error: %v", err)
	}

	log.Debugf("Starting task: %s", newTask.Name())
	newTask.Start()

	reqMQueueID := -1
	respMQueueID := -1

	if reqStream != nil {
		reqMQueueID = 0
		w.mQueues[newTask] = map[uint16]chan task.Message{
			uint16(reqMQueueID): make(chan task.Message, 1024),
		}
	}

	if respStream != nil {
		respMQueueID = 1
		w.mQueues[newTask] = map[uint16]chan task.Message{
			uint16(respMQueueID): make(chan task.Message, 1024),
		}
	}

	builder := flatbuffers.NewBuilder(512)
	methodOff := builder.CreateString(method)

	if reqStream == nil && respStream == nil {
		// no stream
		dataOff := builder.CreateString(reqData)

		custom.NormalRequestInfoStart(builder)
		custom.NormalRequestInfoAddParamsStr(builder, dataOff)
		infoOff := custom.NormalRequestInfoEnd(builder)

		custom.CustomRequestStart(builder)
		custom.CustomRequestAddMethodStr(builder, methodOff)
		custom.CustomRequestAddRequestInfoType(builder,
			custom.RequestInfoNormalRequestInfo)
		custom.CustomRequestAddRequestInfo(builder, infoOff)
		builder.Finish(custom.CustomRequestEnd(builder))
	} else {
		custom.StreamRequestInfoStart(builder)
		custom.StreamRequestInfoAddInQueueId(builder, int32(reqMQueueID))
		custom.StreamRequestInfoAddOutQueueId(builder, int32(respMQueueID))
		infoOff := custom.StreamRequestInfoEnd(builder)

		custom.CustomRequestStart(builder)
		custom.CustomRequestAddMethodStr(builder, methodOff)
		custom.CustomRequestAddRequestInfoType(builder,
			custom.RequestInfoStreamRequestInfo)
		custom.CustomRequestAddRequestInfo(builder, infoOff)
		builder.Finish(custom.CustomRequestEnd(builder))
	}

	if reqStream != nil {
		go func() {
			for msg := range reqStream {
				w.mQueues[newTask][uint16(reqMQueueID)] <- msg
			}
		}()
	}

	if r, err := w.commMgr.SendOutgoingRPCRequest(newTask,
		transport.MethodCustom,
		builder.FinishedBytes()); err != nil {
		return nil, "", fmt.Errorf("error: %v", err)
	} else {
		if len(r.ResponseBytes()) == 0 {
			return newTask, "", nil
		}
		customResp := custom.GetRootAsCustomResponse(r.ResponseBytes(), 0)

		if customResp.ReturnStream() {
			// streaming response
			if _, ok := w.mQueues[newTask][uint16(respMQueueID)]; !ok {
				return nil, "", fmt.Errorf("error: queue not found: %d",
					respMQueueID)
			}

			go func() {
				for msg := range w.mQueues[newTask][uint16(respMQueueID)] {
					respStream <- msg
				}
			}()

			return newTask, "", nil
		} else {
			customRespData := customResp.DataBytes()
			return newTask, string(customRespData), nil
		}
	}
}

func (w *Spearlet) ExecuteTask(funcId int64, funcName string, funcType task.TaskType,
	method, data string, inStream, outStream chan task.Message) (t task.Task, respData string,
	err error) {
	if funcId >= 0 {
		return w.ExecuteTaskById(funcId, funcType, method, data, inStream, outStream)
	}
	if funcName != "" {
		return w.ExecuteTaskByName(funcName, funcType, method, data, inStream, outStream)
	}
	return nil, "", fmt.Errorf("error: invalid task id or name")
}

func (w *Spearlet) handleStream(resp http.ResponseWriter, req *http.Request) {
	var inData string
	var inStream, outStream chan task.Message
	var conn *websocket.Conn
	var err error

	log.Infof("Streaming request")
	conn, err = w.streamUpgrader.Upgrade(resp, req, nil)
	if err != nil {
		respError(resp, fmt.Sprintf("Error: %v", err))
		return
	}

	inStream = make(chan task.Message, 1024)
	outStream = make(chan task.Message, 1024)
	go func() {
		defer conn.Close()
		defer close(inStream)
		for {
			_, msg, err := conn.ReadMessage()
			if err != nil {
				// do not print anything if it is 1000 error
				if websocket.IsCloseError(err, websocket.CloseNormalClosure) {
					return
				}
				log.Errorf("Error reading message: %v", err)
				return
			}
			log.Infof("Received message: %s", msg)
			inStream <- task.Message(msg)
		}
	}()

	// get the function type
	funcType, err := funcType(req)
	if err != nil {
		respError(resp, fmt.Sprintf("Error: %v", err))
		return
	}

	// get the function id
	taskId, errTaskId := funcId(req)
	taskName, errTaskName := funcName(req)
	if errTaskId != nil && errTaskName != nil {
		respError(resp, "Error: taskid or taskname is required")
		return
	}

	t, outData, err := w.ExecuteTask(taskId, taskName, funcType, "handle",
		inData, inStream, outStream)
	if err != nil {
		respError(resp, fmt.Sprintf("Error: %v", err))
		return
	}

	if v, ok := <-outStream; !ok {
		// no stream output
		resp.Write([]byte(outData))
	} else {
		err := conn.WriteMessage(websocket.TextMessage, []byte(v))
		if err != nil {
			log.Errorf("Error writing message: %v", err)
		} else {
			for msg := range outStream {
				err := conn.WriteMessage(websocket.TextMessage, []byte(msg))
				if err != nil {
					log.Errorf("Error writing message: %v", err)
					break
				}
			}
		}
	}

	log.Infof("Terminating task %v", t)
	// terminate the task by sending a signal
	if err := w.commMgr.SendOutgoingRPCSignal(t,
		transport.SignalTerminate,
		[]byte{}); err != nil {
		log.Warnf("Error: %v", err)
	}
	go func() {
		if err := t.Stop(); err != nil {
			log.Warnf("Error stopping task: %v", err)
		}
	}()
}

func (w *Spearlet) handle(resp http.ResponseWriter, req *http.Request) {
	var inData string
	var err error

	buf := make([]byte, common.MaxDataResponseSize)
	n, err := req.Body.Read(buf)
	if err != nil && err != io.EOF {
		log.Errorf("Error reading body: %v", err)
		respError(resp, fmt.Sprintf("Error: %v", err))
		return
	}
	inData = string(buf[:n])

	// get the function type
	funcType, err := funcType(req)
	if err != nil {
		respError(resp, fmt.Sprintf("Error: %v", err))
		return
	}

	// get the function id
	taskId, errTaskId := funcId(req)
	taskName, errTaskName := funcName(req)
	if errTaskId != nil && errTaskName != nil {
		respError(resp, "Error: taskid or taskname is required")
		return
	}

	t, outData, err := w.ExecuteTask(taskId, taskName, funcType, "handle",
		inData, nil, nil)
	if err != nil {
		respError(resp, fmt.Sprintf("Error: %v", err))
		return
	}

	resp.Write([]byte(outData))

	log.Infof("Terminating task %v", t)
	// terminate the task by sending a signal
	if err := w.commMgr.SendOutgoingRPCSignal(t,
		transport.SignalTerminate,
		[]byte{}); err != nil {
		log.Warnf("Error: %v", err)
	}
	go func() {
		if err := t.Stop(); err != nil {
			log.Warnf("Error stopping task: %v", err)
		}
	}()
}

func (w *Spearlet) addRoutes() {
	w.mux.HandleFunc("/health", func(resp http.ResponseWriter,
		req *http.Request) {
		resp.Write([]byte("OK"))
	})
	w.mux.HandleFunc("/", w.handle)
	w.mux.HandleFunc("/{funcId}", w.handle)
	w.mux.HandleFunc("/stream", w.handleStream)
	w.mux.HandleFunc("/stream/{funcId}", w.handleStream)
}

func (w *Spearlet) StartProviderService() {
	log.Infof("Starting provider service")
	// setup gin
	r := gin.Default()
	r.GET("/model", func(c *gin.Context) {
		// list all APIEndpointMap
		c.JSON(http.StatusOK, hostcalls.APIEndpointMap)
	})
	r.GET("/model/:type", func(c *gin.Context) {
		// list all APIEndpointMap with function type `type`
		typ := c.Param("type")
		// convert to int
		t, err := strconv.Atoi(typ)
		if err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": "invalid type"})
			return
		}
		if _, ok := hostcalls.APIEndpointMap[hostcalls.OpenAIFunctionType(t)]; !ok {
			c.JSON(http.StatusBadRequest, gin.H{"error": "invalid type"})
			return
		}
		c.JSON(http.StatusOK,
			hostcalls.APIEndpointMap[hostcalls.OpenAIFunctionType(t)])
	})
	r.POST("/model/:type", func(c *gin.Context) {
		// add or update APIEndpointMap with function type `type` and name `name`
		typ := c.Param("type")
		// convert to int
		t, err := strconv.Atoi(typ)
		if err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": "invalid type"})
			return
		}
		// get the body
		var body hostcalls.APIEndpointInfo
		if err := c.BindJSON(&body); err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": "invalid body"})
			return
		}
		if _, ok := hostcalls.APIEndpointMap[hostcalls.OpenAIFunctionType(t)]; !ok {
			hostcalls.APIEndpointMap[hostcalls.OpenAIFunctionType(t)] =
				[]hostcalls.APIEndpointInfo{}
		}
		hostcalls.APIEndpointMap[hostcalls.OpenAIFunctionType(t)] = append(
			hostcalls.APIEndpointMap[hostcalls.OpenAIFunctionType(t)], body)
		c.JSON(http.StatusOK, gin.H{"status": "success"})
	})

	go func() {
		// convert port to number and increment by 1
		port, err := strconv.Atoi(w.cfg.Port)
		if err != nil {
			log.Fatalf("Error: %v", err)
		}
		port++
		log.Infof("Starting ProviderService server on port %d", port)
		if err := r.Run(fmt.Sprintf("%s:%d", w.cfg.Addr, port)); err != nil {
			log.Fatalf("Failed to start gin server: %v", err)
		}
	}()
}

func (w *Spearlet) StartServer() {
	log.Infof("Starting spearlet on %s:%s", w.cfg.Addr, w.cfg.Port)
	srv := &http.Server{
		Addr:    w.cfg.Addr + ":" + w.cfg.Port,
		Handler: w.mux,
	}
	w.srv = srv
	if w.isSSL {
		log.Infof("SSL Enabled")
		if err := srv.ListenAndServeTLS(w.certFile, w.keyFile); err != nil {
			log.Errorf("Error: %v", err)
		}
	} else {
		log.Infof("SSL Disabled")
		if err := srv.ListenAndServe(); err != nil {
			if err != http.ErrServerClosed {
				log.Errorf("Error: %v", err)
			} else {
				log.Info("Server closed")
			}
		}
	}
}

func (w *Spearlet) Stop() {
	log.Debugf("Stopping spearlet")
	if w.srv != nil {
		w.srv.Shutdown(context.Background())
	}
	w.rtCollection.Cleanup()
}

func SetLogLevel(lvl log.Level) {
	logLevel = lvl
	log.SetLevel(logLevel)
}

func init() {
	log.SetLevel(logLevel)
}

func respError(resp http.ResponseWriter, msg string) {
	log.Warnf("Returning error %s", msg)
	resp.WriteHeader(http.StatusInternalServerError)
	resp.Write([]byte(msg))
}
