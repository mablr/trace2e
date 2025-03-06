#!/bin/bash

grpcurl -plaintext -d '{"file": {"path": "/examples/send_file_index.html"}}' hyper_server:8080 user_api.User/EnableLocalConfidentiality