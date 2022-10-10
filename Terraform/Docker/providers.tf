provider "docker" {
  host = local.is_linux ? "unix:///var/run/docker.sock" : "npipe:////.//pipe//docker_engine"
}