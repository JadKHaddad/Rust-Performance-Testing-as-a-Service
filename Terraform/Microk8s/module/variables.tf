variable "k8s_config_path" {
  type    = string
  default = "/var/snap/microk8s/current/credentials/client.config"
}

variable "namespace" {
  description = "Namespace to be used for all the deployments"
  type        = string
  default     = "performance-testing"
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