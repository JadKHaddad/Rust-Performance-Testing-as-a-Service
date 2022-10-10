terraform {
  required_providers {
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = "= 2.13.1"
    }
    docker = {
      source  = "kreuzwerker/docker"
      version = "2.22.0"
    }
  }
}

provider "kubernetes" {
  config_path = var.k8s_config_path
}

provider "docker" {
  host = local.is_linux ? "unix:///var/run/docker.sock" : "npipe:////.//pipe//docker_engine"
}

# TDOD!: build and push the images
