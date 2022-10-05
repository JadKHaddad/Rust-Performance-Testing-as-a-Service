terraform {
  required_providers {
    docker = {
      source  = "kreuzwerker/docker"
      version = "~> 2.21.0"
    }
  }
}

provider "docker" {
  host = "npipe:////.//pipe//docker_engine" # windows
  # host = "unix:///var/run/docker.sock" # linux

}

locals {
  root_path_tmp       = "/${replace(abspath(path.root), ":", "")}"
  root_path           = replace(local.root_path_tmp, "////", "/")
  context_path        = "../../"
  dockerfiles_path    = "Dockerfiles"
  data_path           = "${local.root_path}/../../Dockerfiles/Performance-Testing-Data"
  container_data_path = "/home/app/Backend/Performance-Testing-Data"
  docker_registry     = "localhost:32000"
}

resource "docker_network" "network" {
  name = "network"
}

resource "docker_image" "redis" {
  name         = "redis:latest"
  keep_locally = false
}

resource "docker_container" "redis" {
  image = docker_image.redis.image_id
  name  = "redis"
  networks_advanced {
    name = docker_network.network.name
  }
  restart = "unless-stopped"
}

resource "docker_image" "builder" {
  name         = "builder:latest"
  keep_locally = true
  build {
    path       = local.context_path
    dockerfile = "${local.dockerfiles_path}/Dockerfile.builder"
  }
}

resource "docker_image" "runner" {
  name         = "runner:latest"
  keep_locally = true
  build {
    path       = local.context_path
    dockerfile = "${local.dockerfiles_path}/Dockerfile.runner"
  }
}

resource "docker_image" "master" {
  name         = "${local.docker_registry}/master-release:latest"
  keep_locally = true
  build {
    path       = local.context_path
    dockerfile = "${local.dockerfiles_path}/Dockerfile.master-release"
  }
}

resource "docker_container" "master" {
  user  = "root"
  image = docker_image.master.image_id
  name  = "master"
  ports {
    internal = 3000
    external = 3000
  }
  volumes {
    container_path = local.container_data_path
    host_path      = local.data_path
  }
  env = ["REDIS_HOST=${docker_container.redis.name}"]
  networks_advanced {
    name = docker_network.network.name
  }
  restart = "unless-stopped"
}

resource "docker_image" "worker" {
  name         = "${local.docker_registry}/worker-release:latest"
  keep_locally = true
  build {
    path       = local.context_path
    dockerfile = "${local.dockerfiles_path}/Dockerfile.worker-release"
  }
}

resource "docker_container" "worker-1" {
  user  = "root"
  image = docker_image.worker.image_id
  name  = "worker-1"
  ports {
    internal = 5000
    external = 5001
  }
  volumes {
    container_path = local.container_data_path
    host_path      = local.data_path
  }
  env = [
    "REDIS_HOST=${docker_container.redis.name}",
    "MASTER_IP=${docker_container.master.name}:${docker_container.master.ports[0].internal}",
    "WORKER_NAME=worker-1:5000"
  ]
  networks_advanced {
    name = docker_network.network.name
  }
  restart = "unless-stopped"
}

resource "docker_container" "worker-2" {
  user  = "root"
  image = docker_image.worker.image_id
  name  = "worker-2"
  ports {
    internal = 5000
    external = 5002
  }
  volumes {
    container_path = local.container_data_path
    host_path      = local.data_path
  }
  env = [
    "REDIS_HOST=${docker_container.redis.name}",
    "MASTER_IP=${docker_container.master.name}:${docker_container.master.ports[0].internal}",
    "WORKER_NAME=worker-2:5000"
  ]
  networks_advanced {
    name = docker_network.network.name
  }
  restart = "unless-stopped"
}

resource "docker_image" "frontend" {
  name         = "${local.docker_registry}/frontend:latest"
  keep_locally = true
  build {
    path       = local.context_path
    dockerfile = "${local.dockerfiles_path}/Dockerfile.frontend"
  }
}

resource "docker_container" "frontend" {
  image = docker_image.frontend.image_id
  name  = "frontend"
  ports {
    internal = 80
    external = 7000
  }
  networks_advanced {
    name = docker_network.network.name
  }
  restart = "unless-stopped"
}

resource "docker_image" "loadbalancer" {
  name         = "${local.docker_registry}/loadbalancer:latest"
  keep_locally = true
  build {
    path       = local.context_path
    dockerfile = "${local.dockerfiles_path}/Dockerfile.loadbalancer"
  }
}

resource "docker_container" "loadbalancer" {
  image = docker_image.loadbalancer.image_id
  name  = "loadbalancer"
  ports {
    internal = 80
    external = 5000
  }
  networks_advanced {
    name = docker_network.network.name
  }
  restart = "unless-stopped"
}

resource "docker_image" "entrypoint" {
  name         = "${local.docker_registry}/entrypoint:latest"
  keep_locally = true
  build {
    path       = local.context_path
    dockerfile = "${local.dockerfiles_path}/Dockerfile.entrypoint"
  }
}

resource "docker_container" "entrypoint" {
  image = docker_image.entrypoint.image_id
  name  = "entrypoint"
  ports {
    internal = 80
    external = 8000
  }
  networks_advanced {
    name = docker_network.network.name
  }
  restart = "unless-stopped"
}
