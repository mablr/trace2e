#!/bin/bash

grpcurl -plaintext -d '{}' localhost:8080 user_api.User/PrintDB