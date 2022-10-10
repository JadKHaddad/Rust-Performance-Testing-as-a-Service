module "module" {
  source = "./module"

  k8s_config_path   = var.k8s_config_path
  namespace         = var.namespace
  pv_local_path     = var.pv_local_path
  worker_count      = var.worker_count
  registry          = var.registry
  image_pull_policy = var.image_pull_policy
}

variable "k8s_config_path" {
  type    = string
  default = "/var/snap/microk8s/current/credentials/client.config"
}

variable "namespace" {
  type    = string
  default = "performance-testing"
}

variable "pv_local_path" {
  type    = string
  default = "/kubernetes/performance-testing/Performance-Testing-Data"
}

variable "worker_count" {
  type    = number
  default = 2
}

variable "registry" {
  type    = string
  default = "localhost:32000"
}

variable "image_pull_policy" {
  type    = string
  default = "Always"
}
