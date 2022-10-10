resource "docker_image" "worker" {
  depends_on = [
    docker_image.runner,
    docker_image.builder
  ]
  name         = "${local.docker_registry}/worker-release:latest"
  keep_locally = true
  build {
    path       = local.paths.context_path
    dockerfile = "${local.paths.dockerfiles_path}/Dockerfile.worker-release"
  }
}

resource "docker_container" "workers" {
  count = 2
  user  = "root"
  image = docker_image.worker.image_id
  name  = "worker-${count.index + 1}"
  ports {
    internal = local.ports.worker_internal_port
    external = local.ports.worker_external_port + count.index + 1
  }
  volumes {
    container_path = local.paths.container_data_path
    host_path      = local.paths.data_path
  }
  env = [
    "REDIS_HOST=${docker_container.redis.name}",
    "MASTER_IP=${docker_container.master.name}:${docker_container.master.ports.0.internal}",
    "WORKER_NAME=worker-${count.index + 1}:${local.ports.worker_internal_port}"
  ]
  networks_advanced {
    name = docker_network.network.name
  }
  restart = local.container_restart_policy
}

resource "docker_image" "loadbalancer" {
  name         = "${local.docker_registry}/loadbalancer:latest"
  keep_locally = true
  build {
    path       = local.paths.context_path
    dockerfile = "${local.paths.dockerfiles_path}/Dockerfile.loadbalancer"
  }
}

resource "docker_container" "loadbalancer" {
  depends_on = [
    docker_container.workers
  ]
  image = docker_image.loadbalancer.image_id
  name  = "loadbalancer"
  ports {
    internal = local.ports.loadbalancer_internal_port
    external = local.ports.loadbalancer_external_port
  }
  networks_advanced {
    name = docker_network.network.name
  }
  restart = local.container_restart_policy
}