# pulld

Simple distributed deployments using GitHub as backend (and other forges in the future)

## Usage

```sh
Usage: pulld [OPTIONS] --backend <BACKEND> --owner <OWNER> --repo <REPO> --ssh_key_file <PATH>

Options:
      --backend <BACKEND>         The backend to use [env: PULLD_BACKEND=] [possible values: github]
      --owner <OWNER>             The owner of the repository to watch for changes [env: PULLD_OWNER=]
      --repo <REPO>               The repository to watch for changes [env: PULLD_REPO=]
      --branch <BRANCH>           Branch to watch for changes [env: PULLD_BRANCH=] [default: main]
      --checkout_path <PATH>      Path where the repository will be checked out locally [env: PULLD_CHECKOUT_PATH=]
      --ssh_key_file <PATH>       Path to the SSH private key file used for git [env: PULLD_SSH_KEY_FILE=]
      --poll_interval <SECONDS>   Time to wait between poll for changes in seconds [env: PULLD_POLL_INTERVAL=] [default: 10]
      --github_token <TOKEN>      Personal access token for authentication [env: PULLD_GITHUB_TOKEN]
      --github_token_file <PATH>  Path to a file containing the personal access token for authentication [env: PULLD_GITHUB_TOKEN_FILE=]
      --host_identifier <NAME>    Identifier of the local host. Defaults to the hostname [env: PULLD_HOST_IDENTIFIER=]
  -h, --help                      Print help
```

## Workflows

.pulld.yaml
```yaml
jobs:
  nixos:
    hosts:
      - my-hostname
    script:
      - |
        if [ "$HOST_OS" = "macos" ]; then
          sudo darwin-rebuild switch
        else
          sudo nixos-rebuild switch
        fi
 ```
