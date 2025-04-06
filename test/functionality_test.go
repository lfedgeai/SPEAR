package test

import (
	"path/filepath"
	"runtime"
	"testing"

	"github.com/lfedgeai/spear/pkg/common"
	"github.com/lfedgeai/spear/spearlet"
	"github.com/lfedgeai/spear/spearlet/task"
	log "github.com/sirupsen/logrus"
)

func TestFunctionality(t *testing.T) {
	// create config
	config := spearlet.NewExecSpearletConfig(true, common.SpearPlatformAddress, []string{}, true)
	w := spearlet.NewSpearlet(config)
	w.Initialize()
	t.Cleanup(func() {
		log.Infof("Stopping spearlet")
		w.Stop()
	})

	res, err := w.RunTask(-1, "pytest-functionality", task.TaskTypeDocker,
		"handle", "",
		nil, nil,
		true, true)
	if err != nil {
		t.Fatalf("Error executing workload: %v", err)
	}
	if len(res) > 1024 {
		res = res[:1024] + "..."
	}
	t.Logf("Workload execution result: %v", res)
}

func TestProcFunctionality(t *testing.T) {
	// get the location of this test file
	_, filename, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatalf("Failed to get the location of this test file")
	}

	dir := filepath.Dir(filename)
	// get ../workload/process/python
	dir = filepath.Join(dir, "..", "workload", "process", "python")
	log.Infof("Directory: %v", dir)

	// create config
	config := spearlet.NewExecSpearletConfig(true, common.SpearPlatformAddress, []string{dir}, false)
	w := spearlet.NewSpearlet(config)
	w.Initialize()
	t.Cleanup(func() {
		log.Infof("Stopping spearlet")
		w.Stop()
	})

	res, err := w.RunTask(-1, "pytest-functionality.py", task.TaskTypeProcess,
		"handle", "",
		nil, nil,
		true, true)
	if err != nil {
		t.Fatalf("Error executing workload: %v", err)
	}
	if len(res) > 1024 {
		res = res[:1024] + "..."
	}
	t.Logf("Workload execution result: %v", res)
}

func TestProcStreamingFunctionality(t *testing.T) {
	// get the location of this test file
	_, filename, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatalf("Failed to get the location of this test file")
	}

	dir := filepath.Dir(filename)
	// get ../workload/process/python
	dir = filepath.Join(dir, "..", "workload", "process", "python")
	log.Infof("Directory: %v", dir)

	// create config
	config := spearlet.NewExecSpearletConfig(true, common.SpearPlatformAddress, []string{dir}, false)
	w := spearlet.NewSpearlet(config)
	w.Initialize()
	t.Cleanup(func() {
		log.Infof("Stopping spearlet")
		w.Stop()
	})

	in_stream := make(chan task.Message, 10)
	out_stream := make(chan task.Message, 10)

	res, err := w.RunTask(-1, "pytest-functionality.py", task.TaskTypeProcess,
		"handle_stream", "",
		in_stream, out_stream,
		true, true)
	if err != nil {
		t.Fatalf("Error executing workload: %v", err)
	}
	if len(res) > 1024 {
		res = res[:1024] + "..."
	}
	t.Logf("Workload execution result: %v", res)
}
