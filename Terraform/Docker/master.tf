resource "docker_image" "master" {
  depends_on = [
    docker_image.runner,
    docker_image.builder
  ]
  name         = "${local.docker_registry}/master-release:latest"
  keep_locally = true
  build {
    path       = local.paths.context_path
    dockerfile = "${local.paths.dockerfiles_path}/Dockerfile.master-release"
  }
}

resource "docker_container" "master" {
  user  = "root"
  image = docker_image.master.image_id
  name  = "master"
  ports {
    internal = local.ports.master_internal_port
    external = local.ports.master_external_port
  }
  volumes {
    container_path = local.paths.container_data_path
    host_path      = local.paths.data_path
  }
  env = ["REDIS_HOST=${docker_container.redis.name}"]
  networks_advanced {
    name = docker_network.network.name
  }
  restart = local.container_restart_policy
}