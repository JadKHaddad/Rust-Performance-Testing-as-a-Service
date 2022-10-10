resource "docker_image" "frontend" {
  name         = "${local.docker_registry}/frontend:latest"
  keep_locally = true
  build {
    path       = local.paths.context_path
    dockerfile = "${local.paths.dockerfiles_path}/Dockerfile.frontend"
  }
}

resource "docker_container" "frontend" {
  image = docker_image.frontend.image_id
  name  = "frontend"
  ports {
    internal = local.ports.frontend_internal_port
    external = local.ports.frontend_external_port
  }
  networks_advanced {
    name = docker_network.network.name
  }
  restart = local.container_restart_policy
}