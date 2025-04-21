package main

import (
	"strings"

	"github.com/lfedgeai/spear/pkg/common"
	"github.com/lfedgeai/spear/pkg/spear/proto/transport"
	spearlet "github.com/lfedgeai/spear/spearlet"
	"github.com/lfedgeai/spear/spearlet/task"
	log "github.com/sirupsen/logrus"
	"github.com/spf13/cobra"

	"os"
)

var (
	execWorkloadName string
	execReqMethod    string
	execReqPayload   string
	execStreaming    bool

	runStartBackendServices bool
	runSpearAddr            string
	runSearchPaths          []string
	runVerbose              bool
	runDebug                bool

	serveAddr string
	servePort string

	// Cert & Key files can be generated for testing using command
	// openssl req -x509 -newkey rsa:2048 -keyout server.key -out server.crt -days 365 -nodes
	serveCertFile string
	serveKeyFile  string

	validChoices = map[string]task.TaskType{
		"docker": task.TaskTypeDocker,
		"file":   task.TaskTypeProcess,
		"dylib":  task.TaskTypeDylib,
		"wasm":   task.TaskTypeWasm,
	}
)

func validateSearchPaths(paths []string) ([]string, error) {
	rtnPaths := make([]string, len(paths))
	// change relative paths to absolute paths
	cwd, err := os.Getwd()
	if err != nil {
		log.Errorf("Error getting current working directory: %v", err)
		return nil, err
	}
	for i, path := range paths {
		if strings.HasPrefix(path, "/") {
			rtnPaths[i] = path
		} else {
			rtnPaths[i] = cwd + "/" + path
		}
	}

	// check if the paths exist
	for _, path := range rtnPaths {
		if _, err := os.Stat(path); os.IsNotExist(err) {
			log.Errorf("Path %s does not exist", path)
			return nil, err
		}
	}
	// check if the paths are directories
	for _, path := range rtnPaths {
		if fi, err := os.Stat(path); err == nil {
			if !fi.IsDir() {
				log.Errorf("Path %s is not a directory", path)
				return nil, err
			}
		} else {
			log.Errorf("Error getting file info for path %s: %v", path, err)
			return nil, err
		}
	}
	return rtnPaths, nil
}

func NewRootCmd() *cobra.Command {
	var rootCmd = &cobra.Command{
		Use:   "spearlet",
		Short: "spearlet is the command line tool for the SPEAR spearlet",
		Run: func(cmd *cobra.Command, args []string) {
			cmd.Help()
		},
	}

	// exec subcommand
	var execCmd = &cobra.Command{
		Use:   "exec",
		Short: "Execute a workload",
		Run: func(cmd *cobra.Command, args []string) {
			if execWorkloadName == "" {
				log.Errorf("Invalid workload name %s", execWorkloadName)
				return
			}
			if execReqMethod == "" {
				log.Errorf("Invalid request method %s", execReqMethod)
				return
			}
			if runSpearAddr == "" {
				runSpearAddr = common.SpearPlatformAddress
			}
			runSearchPaths, err := validateSearchPaths(runSearchPaths)
			if err != nil {
				log.Errorf("Error validating search paths: %v", err)
				return
			}

			// if execWorkloadName is not a number, it is in the format of
			// <scheme>://<name>
			if execWorkloadName != "" && strings.Contains(execWorkloadName, "://") {
				var rtType task.TaskType
				var workloadFullName string

				// split the workload name into scheme and name
				schemeName := strings.Split(execWorkloadName, "://")
				if len(schemeName) != 2 {
					log.Errorf("Invalid workload name %s", execWorkloadName)
					return
				}
				// check if the scheme is valid
				if rtt, ok := validChoices[strings.ToLower(schemeName[0])]; !ok {
					log.Errorf("Invalid workload scheme %s", schemeName[0])
					return
				} else {
					if rtt == task.TaskTypeUnknown {
						log.Errorf("Invalid workload scheme %s", schemeName[0])
						return
					}
					rtType = rtt
					workloadFullName = schemeName[1]
				}

				log.Infof("Executing workload %s with runtime type %v",
					workloadFullName, rtType)
				// set log level
				if runVerbose {
					spearlet.SetLogLevel(log.DebugLevel)
				}

				// create config
				config := spearlet.NewExecSpearletConfig(runDebug, runSpearAddr,
					runSearchPaths, runStartBackendServices)
				w := spearlet.NewSpearlet(config)
				w.Initialize()
				defer func() {
					w.Stop()
				}()

				var inStream chan task.Message
				var outStream chan task.Message
				if execStreaming {
					inStream = make(chan task.Message, 128)
					outStream = make(chan task.Message, 128)
					// get input from stdin until ctrl-d or ctrl-c
					// line separated
					go func() {
						defer close(inStream)
						for {
							buf := make([]byte, 1024)
							n, err := os.Stdin.Read(buf)
							if err != nil {
								break
							}
							inStream <- task.Message(buf[:n])
						}
					}()

					// print to stdout
					go func() {
						for msg := range outStream {
							os.Stdout.Write(msg)
						}
					}()
				}
				t, outData, err := w.ExecuteTask(-1, workloadFullName, rtType,
					execReqMethod, execReqPayload, inStream, outStream)
				if err != nil {
					log.Errorf("Error executing workload: %v", err)
					return
				}
				if outData != "" {
					log.Infof("Workload execution output: %v", outData)
				}

				log.Infof("Terminating task %v", t)
				// terminate the task by sending a signal
				if err := w.CommunicationManager().SendOutgoingRPCSignal(t,
					transport.SignalTerminate,
					[]byte{}); err != nil {
					log.Warnf("Error: %v", err)
				}
				// stop the task with out running in a goroutine to avoid
				// existing without terminating the workload
				if err := t.Stop(); err != nil {
					log.Warnf("Error stopping task: %v", err)
				}
			}
		},
	}

	possibleOptions := []string{}
	for k := range validChoices {
		possibleOptions = append(possibleOptions, k)
	}
	// workload name
	execCmd.PersistentFlags().StringVarP(&execWorkloadName, "name", "n", "",
		"workload name. It can be in the format of <scheme>://<workload_name> or <workload_id>,"+
			" where scheme can be one of "+strings.Join(possibleOptions, ", "))
	// workload request payload
	execCmd.PersistentFlags().StringVarP(&execReqMethod, "method", "m", "handle",
		"default method to invoke")
	execCmd.PersistentFlags().StringVarP(&execReqPayload, "payload", "p", "", "request payload")
	// streaming flag
	execCmd.PersistentFlags().BoolVarP(&execStreaming, "streaming", "S", false,
		"switch to streaming call to the workload")
	rootCmd.AddCommand(execCmd)

	var serveCmd = &cobra.Command{
		Use:   "serve",
		Short: "Start the spearlet server",
		Run: func(cmd *cobra.Command, args []string) {
			// set log level
			if runVerbose {
				spearlet.SetLogLevel(log.DebugLevel)
			}

			if runSpearAddr == "" {
				runSpearAddr = common.SpearPlatformAddress
			}
			runSearchPaths, err := validateSearchPaths(runSearchPaths)
			if err != nil {
				log.Errorf("Error validating search paths: %v", err)
				return
			}

			// create config
			config, err := spearlet.NewServeSpearletConfig(serveAddr, servePort, runSearchPaths,
				runDebug, runSpearAddr, serveCertFile, serveKeyFile, runStartBackendServices)
			if err != nil {
				log.Errorf("Error creating spearlet config: %v", err)
				return
			}
			w := spearlet.NewSpearlet(config)
			w.Initialize()
			w.StartProviderService()
			w.StartServer()
		},
	}
	// addr flag
	serveCmd.PersistentFlags().StringVarP(&serveAddr, "addr", "a", "localhost",
		"address of the server")
	// port flag
	serveCmd.PersistentFlags().StringVarP(&servePort, "port", "p", "8080", "port of the server")
	// cert file flag
	serveCmd.PersistentFlags().StringVarP(&serveCertFile, "ssl-cert", "c", "", "SSL certificate file")
	// key file flag
	serveCmd.PersistentFlags().StringVarP(&serveKeyFile, "ssl-key", "k", "", "SSL key file")
	rootCmd.AddCommand(serveCmd)

	// spear platform address for workload to connect
	rootCmd.PersistentFlags().StringVarP(&runSpearAddr, "spear-addr", "s", os.Getenv("SPEAR_RPC_ADDR"),
		"SPEAR platform address for workload RPC")
	// search path
	rootCmd.PersistentFlags().StringArrayVarP(&runSearchPaths, "search-path", "L", []string{},
		"search path list for the spearlet")
	// verbose flag
	rootCmd.PersistentFlags().BoolVarP(&runVerbose, "verbose", "v", false, "verbose output")
	// debug flag
	rootCmd.PersistentFlags().BoolVarP(&runDebug, "debug", "d", false, "debug mode")
	// backend service
	rootCmd.PersistentFlags().BoolVarP(&runStartBackendServices, "backend-services", "b", false,
		"start backend services")
	// version flag
	rootCmd.Version = common.Version
	return rootCmd
}

func main() {
	NewRootCmd().Execute()
}
