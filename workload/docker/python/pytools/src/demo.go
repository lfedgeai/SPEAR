package main

import (
	"bufio"
	"bytes"
	"fmt"
	"net/http"
	"os"

	"github.com/lfedgeai/spear/pkg/tools/docker"
	log "github.com/sirupsen/logrus"
)

func init() {
	log.SetLevel(log.DebugLevel)
}

func main() {
	// read user input
	reader := bufio.NewReader(os.Stdin)
	fmt.Print("Message to LLM: ")

	input, err := reader.ReadString('\n')
	if err != nil {
		panic("reader.ReadString failed: " + err.Error())
	}

	// setup test environment
	s := docker.NewTestSetup()
	defer s.TearDown()

	// send a http request to the server and check the response
	client := &http.Client{}
	req, err := http.NewRequest("GET", "http://localhost:8080", bytes.NewBuffer(
		[]byte(input),
	))

	if err != nil {
		panic("http.NewRequest failed: " + err.Error())
	}

	// add headers
	req.Header.Add("Content-Type", "application/json")
	req.Header.Add("Accept", "application/json")
	req.Header.Add("Spear-Func-Id", "5")
	req.Header.Add("Spear-Func-Type", "1")

	// send the request
	resp, err := client.Do(req)
	if err != nil {
		panic("client.Do failed: " + err.Error())
	}

	// print the response
	buf := new(bytes.Buffer)
	buf.ReadFrom(resp.Body)

	respData := buf.Bytes()
	log.Debugf("Received response: %s", respData)
}
