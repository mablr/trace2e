#!/bin/bash

grpcurl -plaintext -d '{}' hyper_server:8080 user_api.User/PrintDB