terraform {
  required_providers {
    docker = {
      source  = "kreuzwerker/docker"
      version = "~> 2.15.0"
    }
  }
}

provider "docker" {
  host = "npipe:////.//pipe//docker_engine" # windows
  # host = "unix:///var/run/docker.sock" # linux

}

locals {
  root_path_tmp = "/${replace(abspath(path.root), ":", "")}"
  root_path     = replace(local.root_path_tmp, "////", "/")
  context_path  = "../"
  dockerfiles_path = "Dockerfiles"
  data_path = "${local.root_path}/../Dockerfiles/Performance-Testing-Data"
  container_data_path = "/home/app/Backend/Performance-Testing-Data"
}

resource "docker_network" "network" {
  name = "network"
}

resource "docker_image" "redis" {
  name         = "redis:latest"
  keep_locally = false

}

resource "docker_container" "redis" {
  image = docker_image.redis.latest
  name  = "redis"
  networks_advanced {
    name = docker_network.network.name
  }
}

resource "docker_image" "master" {
  name         = "localhost:32000/master-release:latest"
  keep_locally = true
  build {
    path       = local.context_path
    dockerfile = "${local.dockerfiles_path}/Dockerfile.master-release"
  }
}

resource "docker_container" "master" {
  user  = "root"
  image = docker_image.master.latest
  name  = "master"
  ports {
    internal = 3000
    external = 3000
  }
  volumes {
    container_path = local.container_data_path
    host_path      = local.data_path
  }
  env = ["REDIS_HOST=redis"]
  networks_advanced {
    name = docker_network.network.name
  }
}

resource "docker_image" "worker" {
  name         = "localhost:32000/worker-release"
  keep_locally = true
  build {
    path       = local.context_path
    dockerfile = "${local.dockerfiles_path}/Dockerfile.worker-release"
  }
}

resource "docker_container" "worker-1" {
  user  = "root"
  image = docker_image.worker.latest
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
    "REDIS_HOST=redis",
    "MASTER_IP=master:3000",
    "WORKER_NAME=worker-1:5000"
  ]
  networks_advanced {
    name = docker_network.network.name
  }
}

resource "docker_container" "worker-2" {
  user  = "root"
  image = docker_image.worker.latest
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
    "REDIS_HOST=redis",
    "MASTER_IP=master:3000",
    "WORKER_NAME=worker-2:5000"
  ]
  networks_advanced {
    name = docker_network.network.name
  }
}

resource "docker_image" "frontend" {
  name         = "localhost:32000/frontend"
  keep_locally = true
  build {
    path       = local.context_path
    dockerfile = "${local.dockerfiles_path}/Dockerfile.frontend"
  }
}

resource "docker_container" "frontend" {
  image = docker_image.frontend.latest
  name  = "frontend"
  ports {
    internal = 80
    external = 7000
  }
  networks_advanced {
    name = docker_network.network.name
  }
}

resource "docker_image" "loadbalancer" {
  name         = "localhost:32000/loadbalancer"
  keep_locally = true
  build {
    path       = local.context_path
    dockerfile = "${local.dockerfiles_path}/Dockerfile.loadbalancer"
  }
}

resource "docker_container" "loadbalancer" {
  image = docker_image.loadbalancer.latest
  name  = "loadbalancer"
  ports {
    internal = 80
    external = 5000
  }
  networks_advanced {
    name = docker_network.network.name
  }
}

resource "docker_image" "entrypoint" {
  name         = "localhost:32000/entrypoint"
  keep_locally = true
  build {
    path       = local.context_path
    dockerfile ="${local.dockerfiles_path}/Dockerfile.entrypoint"
  }
}

resource "docker_container" "entrypoint" {
  image = docker_image.entrypoint.latest
  name  = "entrypoint"
  ports {
    internal = 80
    external = 8000
  }
  networks_advanced {
    name = docker_network.network.name
  }
}
