#!/bin/bash

grpcurl -plaintext -d '{"file": {"path": "/template.html"}}' hyper_server:8080 user_api.User/EnableLocalIntegrity