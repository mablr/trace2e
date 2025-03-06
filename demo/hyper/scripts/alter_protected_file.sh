#!/bin/bash
# Reset template.html
printf "# Preparing demo: ... "
./hyper_client_post http://hyper_server:1340/reset_template

# Upload legit file
## (no trace2e)
printf "\n\n# Uploading legit file (no trace2e)\n"
./hyper_client_post http://hyper_server:1340/upload/file
## (with trace2e)
printf "\n\n# Uploading legit file (with trace2e)\n"
./hypere2e_client_post http://hyper_server:1339/upload/file_e2e

# Alter template.html 
## (no trace2e)
printf "\n\n# Try to alter template.html (no trace2e)\n"
./hyper_client_post http://hyper_server:1340/upload/template.html
./hyper_client_post http://hyper_server:1340/reset_template
## (with trace2e) Must fail if "enable_file_local_integrity.sh" was executed before
printf "\n\n# Try to alter template.html (with trace2e), must fail if local integrity is enabled\n"
./hypere2e_client_post http://hyper_server:1339/upload/template.html