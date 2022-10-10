terraform {
  required_providers {
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = "= 2.13.1"
    }
  }
}

provider "kubernetes" {
  config_path = "/var/snap/microk8s/current/credentials/client.config"
}

locals {
  ports = {
    redis = {
      service = {
        port        = 6379
        target_port = 6379
      }
      container = {
        container_port = 6379
      }
    }
    master = {
      service = {
        port        = 3000
        target_port = 3000
      }
      container = {
        container_port = 3000
      }
    }
    workers = {
      service = {
        port        = 5000
        target_port = 5000
      }
      loadbalancer = {
        port        = 5000
        target_port = 5000
      }
      container = {
        container_port = 5000
      }
    }
    frontend = {
      service = {
        port        = 80
        target_port = 80
      }
      container = {
        container_port = 80
      }
    }
  }
  consts = {
    redis_host  = "REDIS_HOST"
    master_ip   = "MASTER_IP"
    worker_name = "WORKER_NAME"
  }
  paths = {
    container = {
      mount_path = "/home/app/Backend/Performance-Testing-Data"
    },
  }
  services = {
    redis = {
      name = "redis-service"
    },
    master = {
      name = "master-service"
    },
    frontend = {
      name = "frontend-service"
    },
    workers = {
      loadbalancer = {
        name = "worker-loadbalancer"
      }
    }
  }
  deployments = {
    redis = {
      name = "redis-deployment"
    },
    master = {
      name = "master-deployment"
    },
    frontend = {
      name = "frontend-deployment"
    },
  }
  session_affinity  = "None"

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
    type = number
    default = 2
}

variable "registry" {
  type = string
  default = "localhost:32000"
}

variable "image_pull_policy" {
  type = string
  default = "Always"
}

resource "kubernetes_namespace" "main_namespace" {
  metadata {
    name = var.namespace
  }
}

resource "kubernetes_persistent_volume" "pv_volume" {
  metadata {
    name = "pv-volume"
    labels = {
      type = "local"
    }
  }
  spec {
    storage_class_name = "manual"
    capacity = {
      storage = "10Gi"
    }
    access_modes = ["ReadWriteOnce"]
    persistent_volume_source {
      host_path {
        path = var.pv_local_path
      }
    }
  }
}

resource "kubernetes_persistent_volume_claim" "pv_claim" {
  depends_on = [
    kubernetes_persistent_volume.pv_volume
  ]
  metadata {
    name      = "pv-claim"
    namespace = kubernetes_namespace.main_namespace.metadata.0.name
  }
  spec {
    storage_class_name = "manual"
    access_modes       = ["ReadWriteOnce"]
    resources {
      requests = {
        storage = "3Gi"
      }
    }
  }
}

resource "kubernetes_config_map" "configmap" {
  metadata {
    name      = "configmap"
    namespace = kubernetes_namespace.main_namespace.metadata.0.name
  }
  data = {
    redis_host = local.services.redis.name
    master_ip  = "${local.services.master.name}:${local.ports.master.service.port}"
  }
}

resource "kubernetes_service" "redis_service" {
  metadata {
    name      = local.services.redis.name
    namespace = kubernetes_namespace.main_namespace.metadata.0.name
  }
  spec {
    selector = {
      app = kubernetes_deployment.redis_deployment.metadata.0.labels.app
    }
    session_affinity = local.session_affinity
    port {
      name        = "http"
      protocol    = "TCP"
      port        = local.ports.redis.service.port
      target_port = local.ports.redis.service.target_port
    }
  }
}

resource "kubernetes_deployment" "redis_deployment" {
  metadata {
    name      = local.deployments.redis.name
    namespace = kubernetes_namespace.main_namespace.metadata.0.name
    labels = {
      app = "redis"
    }
  }

  spec {
    replicas = 1
    selector {
      match_labels = {
        app = "redis"
      }
    }
    template {
      metadata {
        annotations = {
          "sidecar.istio.io/inject" = "false"
        }
        labels = {
          app = "redis"
        }
      }
      spec {
        container {
          image             = "redis"
          name              = "redis"
          image_pull_policy = var.image_pull_policy
          port {
            container_port = local.ports.redis.container.container_port
          }
        }
      }
    }
  }
}

resource "kubernetes_service" "master_service" {
  metadata {
    name      = local.services.master.name
    namespace = kubernetes_namespace.main_namespace.metadata.0.name
  }
  spec {
    selector = {
      app = kubernetes_deployment.master_deployment.metadata.0.labels.app
    }
    session_affinity = local.session_affinity
    port {
      name        = "http"
      protocol    = "TCP"
      port        = local.ports.master.service.port
      target_port = local.ports.master.service.target_port
    }
  }
}

resource "kubernetes_deployment" "master_deployment" {
  depends_on = [
    kubernetes_persistent_volume.pv_volume,
    kubernetes_config_map.configmap
  ]
  metadata {
    name      = local.deployments.master.name
    namespace = kubernetes_namespace.main_namespace.metadata.0.name
    labels = {
      app = "master"
    }
  }
  spec {
    replicas = 1
    selector {
      match_labels = {
        app = "master"
      }
    }
    template {
      metadata {
        labels = {
          app = "master"
        }
      }
      spec {
        security_context {
          run_as_user  = 0
          run_as_group = 0
          fs_group     = 0
        }
        volume {
          name = "task-pv-storage"
          persistent_volume_claim {
            claim_name = kubernetes_persistent_volume_claim.pv_claim.metadata.0.name
          }
        }
        container {
          image             = "${var.registry}/master-release:latest"
          name              = "master"
          image_pull_policy = var.image_pull_policy
          port {
            container_port = local.ports.master.container.container_port
          }
          env {
            name = local.consts.redis_host
            value_from {
              config_map_key_ref {
                name = kubernetes_config_map.configmap.metadata.0.name
                key  = "redis_host"
              }
            }
          }
          volume_mount {
            mount_path = local.paths.container.mount_path
            name       = "task-pv-storage"
          }
          security_context {
            allow_privilege_escalation = false
          }
        }
      }
    }
  }
}

resource "kubernetes_service" "worker_loadbalancer" {
  metadata {
    name      = local.services.workers.loadbalancer.name
    namespace = kubernetes_namespace.main_namespace.metadata.0.name
  }
  spec {
    selector = {
      label = "worker"
    }
    session_affinity = local.session_affinity
    port {
      name        = "http"
      protocol    = "TCP"
      port        = local.ports.workers.loadbalancer.port
      target_port = local.ports.workers.loadbalancer.target_port
    }
  }
}

resource "kubernetes_service" "workers_service" {
  count = var.worker_count
  metadata {
    name      = "worker-${count.index + 1}-service"
    namespace = kubernetes_namespace.main_namespace.metadata.0.name
  }
  spec {
    selector = {
      app = kubernetes_deployment.workers_deployment[count.index].metadata.0.labels.app
    }
    session_affinity = local.session_affinity
    port {
      name     = "http"
      protocol = "TCP"
      port     = local.ports.workers.service.port

      target_port = local.ports.workers.service.target_port
    }
  }
}

resource "kubernetes_deployment" "workers_deployment" {
  depends_on = [
    kubernetes_persistent_volume.pv_volume,
    kubernetes_config_map.configmap
  ]
  count = var.worker_count
  metadata {
    name      = "worker-${count.index + 1}-deployment"
    namespace = kubernetes_namespace.main_namespace.metadata.0.name
    labels = {
      app = "worker-${count.index + 1}"
    }
  }
  spec {
    replicas = 1
    selector {
      match_labels = {
        app = "worker-${count.index + 1}"
      }
    }
    template {
      metadata {
        labels = {
          app   = "worker-${count.index + 1}"
          label = "worker"
        }
      }
      spec {
        security_context {
          run_as_user  = 0
          run_as_group = 0
          fs_group     = 0
        }
        volume {
          name = "task-pv-storage"
          persistent_volume_claim {
            claim_name = kubernetes_persistent_volume_claim.pv_claim.metadata.0.name
          }
        }
        container {
          image             = "${var.registry}/worker-release:latest"
          name              = "worker-${count.index + 1}"
          image_pull_policy = var.image_pull_policy
          port {
            container_port = local.ports.workers.container.container_port
          }
          env {
            name = local.consts.redis_host
            value_from {
              config_map_key_ref {
                name = kubernetes_config_map.configmap.metadata.0.name
                key  = "redis_host"
              }
            }
          }
          env {
            name = local.consts.master_ip
            value_from {
              config_map_key_ref {
                name = kubernetes_config_map.configmap.metadata.0.name
                key  = "master_ip"
              }
            }
          }
          env {
            name  = local.consts.worker_name
            value = kubernetes_service.workers_service[count.index].metadata.0.name
          }
          volume_mount {
            mount_path = local.paths.container.mount_path
            name       = "task-pv-storage"
          }
          security_context {
            allow_privilege_escalation = false
          }
        }
      }
    }
  }
}

resource "kubernetes_service" "frontend_service" {
  metadata {
    name      = local.services.frontend.name
    namespace = kubernetes_namespace.main_namespace.metadata.0.name
  }
  spec {
    selector = {
      app = kubernetes_deployment.frontend_deployment.metadata.0.labels.app
    }
    session_affinity = local.session_affinity
    port {
      name     = "http"
      protocol = "TCP"
      port     = local.ports.frontend.service.port

      target_port = local.ports.frontend.service.target_port
    }
  }
}

resource "kubernetes_deployment" "frontend_deployment" {
  metadata {
    name      = local.deployments.frontend.name
    namespace = kubernetes_namespace.main_namespace.metadata.0.name
    labels = {
      app = "frontend"
    }
  }
  spec {
    replicas = 1
    selector {
      match_labels = {
        app = "frontend"
      }
    }
    template {
      metadata {
        labels = {
          app = "frontend"
        }
      }
      spec {
        container {
          image             = "${var.registry}/frontend:latest"
          name              = "frontend"
          image_pull_policy = var.image_pull_policy
          port {
            container_port = local.ports.frontend.container.container_port
          }
        }
      }
    }
  }
}

resource "kubernetes_ingress_v1" "ingress" {
  metadata {
    name = "ingress"
    annotations = {
      "nginx.ingress.kubernetes.io/rewrite-target" = "/$1"
      "kubernetes.io/ingress.class"                = "public"
    }
    namespace = kubernetes_namespace.main_namespace.metadata.0.name
  }
  spec {
    rule {
      http {
        path {
          path      = "/(explore.*)"
          path_type = "Prefix"
          backend {
            service {
              name = kubernetes_service.master_service.metadata.0.name
              port {
                number = kubernetes_service.master_service.spec.0.port.0.port
              }
            }
          }
        }
        path {
          path      = "/api/master/(.*)"
          path_type = "Prefix"
          backend {
            service {
              name = kubernetes_service.master_service.metadata.0.name
              port {
                number = kubernetes_service.master_service.spec.0.port.0.port
              }
            }
          }
        }
        path {
          path      = "/api/worker/(.*)"
          path_type = "Prefix"
          backend {
            service {
              name = kubernetes_service.worker_loadbalancer.metadata.0.name
              port {
                number = kubernetes_service.worker_loadbalancer.spec.0.port.0.port
              }
            }
          }
        }
        path {
          path      = "/(.*)"
          path_type = "Prefix"
          backend {
            service {
              name = kubernetes_service.frontend_service.metadata.0.name
              port {
                number = kubernetes_service.frontend_service.spec.0.port.0.port
              }
            }
          }
        }
      }
    }
  }
}

# TDOD!: build and push the images
