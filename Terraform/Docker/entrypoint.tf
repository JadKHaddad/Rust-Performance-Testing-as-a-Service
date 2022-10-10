resource "docker_image" "entrypoint" {
  name         = "${local.docker_registry}/entrypoint:latest"
  keep_locally = true
  build {
    path       = local.paths.context_path
    dockerfile = "${local.paths.dockerfiles_path}/Dockerfile.entrypoint"
  }
}

resource "docker_container" "entrypoint" {
  depends_on = [
    docker_container.master,
    docker_container.loadbalancer,
    docker_container.frontend
  ]
  image = docker_image.entrypoint.image_id
  name  = "entrypoint"
  ports {
    internal = local.ports.entrypoint_internal_port
    external = local.ports.entrypoint_external_port
  }
  networks_advanced {
    name = docker_network.network.name
  }
  restart = local.container_restart_policy
}
