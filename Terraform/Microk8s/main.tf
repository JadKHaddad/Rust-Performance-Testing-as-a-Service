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
  namespace = "performance-testing"
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
    pv = {
      host_path = "/kubernetes/performance-testing/Performance-Testing-Data"
    }
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
  registry          = "localhost:32000"
  session_affinity  = "None"
  image_pull_policy = "Always"
  available_workers = {
    worker_1 = {
    },
    worker_2 = {
    }
  }
}

resource "kubernetes_namespace" "performance_testing" {
  metadata {
    name = local.namespace
  }
}

resource "kubernetes_persistent_volume_claim" "pv_claim" {
  depends_on = [
    kubernetes_persistent_volume.pv_volume
  ]
  metadata {
    name      = "pv-claim"
    namespace = kubernetes_namespace.performance_testing.metadata.0.name
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
        path = local.paths.pv.host_path
      }
    }
  }
}

resource "kubernetes_config_map" "configmap" {
  metadata {
    name      = "configmap"
    namespace = kubernetes_namespace.performance_testing.metadata.0.name
  }
  data = {
    redis_host = local.services.redis.name
    master_ip  = "${local.services.master.name}:${local.ports.master.service.port}"
  }
}

resource "kubernetes_service" "redis_service" {
  metadata {
    name      = local.services.redis.name
    namespace = kubernetes_namespace.performance_testing.metadata.0.name
  }
  spec {
    selector = {
      app = kubernetes_deployment.redis_deployment.metadata.0.labels.app
    }
    session_affinity = local.session_affinity
    port {
      name        = "http"
      port        = local.ports.redis.service.port
      protocol    = "TCP"
      target_port = local.ports.redis.service.target_port
    }
  }
}

resource "kubernetes_deployment" "redis_deployment" {
  metadata {
    name      = local.deployments.redis.name
    namespace = kubernetes_namespace.performance_testing.metadata.0.name
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
          "sidecar.istio.io/inject" = "false" #
        }
        labels = {
          app = "redis"
        }
      }
      spec {
        container {
          image             = "redis"
          name              = "redis"
          image_pull_policy = local.image_pull_policy
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
    namespace = kubernetes_namespace.performance_testing.metadata.0.name
  }
  spec {
    selector = {
      app = kubernetes_deployment.master_deployment.metadata.0.labels.app
    }
    session_affinity = local.session_affinity
    port {
      name        = "http"
      port        = local.ports.master.service.port
      protocol    = "TCP"
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
    namespace = kubernetes_namespace.performance_testing.metadata.0.name
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
            claim_name = "pv-claim"
          }
        }
        container {
          image             = "${local.registry}/master-release:latest"
          name              = "master"
          image_pull_policy = local.image_pull_policy
          port {
            container_port = local.ports.master.container.container_port
          }
          env {
            name = "REDIS_HOST"
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
    namespace = kubernetes_namespace.performance_testing.metadata.0.name
  }
  spec {
    selector = {
      label = "worker"
    }
    session_affinity = local.session_affinity
    port {
      name        = "http"
      port        = local.ports.workers.loadbalancer.port
      protocol    = "TCP"
      target_port = local.ports.workers.loadbalancer.target_port
    }
  }
}

resource "kubernetes_service" "workers_service" {
  for_each = local.available_workers
  metadata {
    name      = "worker-${index(keys(local.available_workers), each.key) + 1}-service"
    namespace = kubernetes_namespace.performance_testing.metadata.0.name
  }
  spec {
    selector = {
      app = kubernetes_deployment.workers_deployment[each.key].metadata.0.labels.app
    }
    session_affinity = local.session_affinity
    port {
      name        = "http"
      port        = local.ports.workers.service.port
      protocol    = "TCP"
      target_port = local.ports.workers.service.target_port
    }
  }
}

resource "kubernetes_deployment" "workers_deployment" {
  depends_on = [
    kubernetes_persistent_volume.pv_volume,
    kubernetes_config_map.configmap
  ]
  for_each = local.available_workers
  metadata {
    name      = "worker-${index(keys(local.available_workers), each.key) + 1}-deployment"
    namespace = kubernetes_namespace.performance_testing.metadata.0.name
    labels = {
      app = "worker-${index(keys(local.available_workers), each.key) + 1}"
    }
  }
  spec {
    replicas = 1
    selector {
      match_labels = {
        app = "worker-${index(keys(local.available_workers), each.key) + 1}"
      }
    }
    template {
      metadata {
        labels = {
          app   = "worker-${index(keys(local.available_workers), each.key) + 1}"
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
            claim_name = "pv-claim"
          }
        }
        container {
          image             = "${local.registry}/worker-release:latest"
          name              = "worker-${index(keys(local.available_workers), each.key) + 1}"
          image_pull_policy = local.image_pull_policy
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
            value = "worker-${index(keys(local.available_workers), each.key) + 1}-service:5000"
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
    namespace = kubernetes_namespace.performance_testing.metadata.0.name
  }
  spec {
    selector = {
      app = kubernetes_deployment.frontend_deployment.metadata.0.labels.app
    }
    session_affinity = local.session_affinity
    port {
      name        = "http"
      port        = local.ports.frontend.service.port
      protocol    = "TCP"
      target_port = local.ports.frontend.service.target_port
    }
  }
}

resource "kubernetes_deployment" "frontend_deployment" {
  metadata {
    name      = local.deployments.frontend.name
    namespace = kubernetes_namespace.performance_testing.metadata.0.name
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
          image             = "${local.registry}/frontend:latest"
          name              = "frontend"
          image_pull_policy = local.image_pull_policy
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
    namespace = kubernetes_namespace.performance_testing.metadata.0.name
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
