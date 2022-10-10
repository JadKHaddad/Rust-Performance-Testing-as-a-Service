terraform {
  required_providers {
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = "= 2.13.1"
    }
  }
}

provider "kubernetes" {
  config_path = var.k8s_config_path
}

# TDOD!: build and push the images
