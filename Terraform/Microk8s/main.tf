provider "kubernetes" {
  config_path = "/var/snap/microk8s/current/credentials/client.config"
}

resource "kubernetes_namespace" "performance-testing" {
  metadata {
    name = "performance-testing"
  }
}

resource "kubernetes_persistent_volume_claim" "pv_claim" {
  metadata {
    name      = "pv-claim"
    namespace = "performance-testing"
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
        path = "/kubernetes/performance-testing/Performance-Testing-Data"
      }
    }
  }
}

resource "kubernetes_config_map" "configmap" {
  metadata {
    name      = "configmap"
    namespace = "performance-testing"
  }
  data = {
    redis_host = "redis-service"
    master_ip  = "master-service:3000"
  }
}

resource "kubernetes_service" "redis_service" {
  metadata {
    name      = "redis-service"
    namespace = "performance-testing"
  }
  spec {
    selector = {
      app = kubernetes_deployment.redis_deployment.metadata.0.labels.app
    }
    session_affinity = "None"
    port {
      name        = "http"
      port        = 6379
      protocol    = "TCP"
      target_port = 6379
    }
    type = "LoadBalancer"
  }
}

resource "kubernetes_deployment" "redis_deployment" {
  metadata {
    name      = "redis-deployment"
    namespace = "performance-testing"
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
          image_pull_policy = "Always"
          port {
            container_port = 6379
          }
        }
      }
    }
  }
}

resource "kubernetes_service" "master_service" {
  metadata {
    name      = "master-service"
    namespace = "performance-testing"
  }
  spec {
    selector = {
      app = kubernetes_deployment.master_deployment.metadata.0.labels.app
    }
    session_affinity = "None"
    port {
      name        = "http"
      port        = 3000
      protocol    = "TCP"
      target_port = 3000
    }
    type = "LoadBalancer"
  }
}

resource "kubernetes_deployment" "master_deployment" {
  metadata {
    name      = "master-deployment"
    namespace = "performance-testing"
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
          image             = "localhost:32000/master-release:latest"
          name              = "master"
          image_pull_policy = "Always"
          port {
            container_port = 3000
          }
          env {
            name = "REDIS_HOST"
            value_from {
              config_map_key_ref {
                name = "configmap"
                key  = "redis_host"
              }
            }
          }
          volume_mount {
            mount_path = "/home/app/Backend/Performance-Testing-Data"
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
    name      = "worker-loadbalancer"
    namespace = "performance-testing"
  }
  spec {
    selector = {
      label = "worker"
    }
    session_affinity = "None"
    port {
      name        = "http"
      port        = 5000
      protocol    = "TCP"
      target_port = 5000
    }
    type = "LoadBalancer"
  }
}

resource "kubernetes_service" "worker_1_service" {
  metadata {
    name      = "worker-1-service"
    namespace = "performance-testing"
  }
  spec {
    selector = {
      app = kubernetes_deployment.worker_1_deployment.metadata.0.labels.app
    }
    session_affinity = "None"
    port {
      name        = "http"
      port        = 5000
      protocol    = "TCP"
      target_port = 5000
    }
    type = "LoadBalancer"
  }
}

resource "kubernetes_deployment" "worker_1_deployment" {
  metadata {
    name      = "worker-1-deployment"
    namespace = "performance-testing"
    labels = {
      app = "worker-1"
    }
  }
  spec {
    replicas = 1
    selector {
      match_labels = {
        app = "worker-1"
      }
    }
    template {
      metadata {
        labels = {
          app   = "worker-1"
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
          image             = "localhost:32000/worker-release:latest"
          name              = "worker-1"
          image_pull_policy = "Always"
          port {
            container_port = 5000
          }
          env {
            name = "REDIS_HOST"
            value_from {
              config_map_key_ref {
                name = "configmap"
                key  = "redis_host"
              }
            }
          }
          env {
            name = "MASTER_IP"
            value_from {
              config_map_key_ref {
                name = "configmap"
                key  = "master_ip"
              }
            }
          }
          env {
            name  = "WORKER_NAME"
            value = "worker-1-service:5000"
          }
          volume_mount {
            mount_path = "/home/app/Backend/Performance-Testing-Data"
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

## worker2 ##

resource "kubernetes_service" "frontend_service" {
  metadata {
    name      = "frontend-service"
    namespace = "performance-testing"
  }
  spec {
    selector = {
      app = kubernetes_deployment.frontend_deployment.metadata.0.labels.app
    }
    session_affinity = "None"
    port {
      name        = "http"
      port        = 80
      protocol    = "TCP"
      target_port = 80
    }
    type = "LoadBalancer"
  }
}

resource "kubernetes_deployment" "frontend_deployment" {
  metadata {
    name      = "frontend-deployment"
    namespace = "performance-testing"
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
          image             = "localhost:32000/frontend:latest"
          name              = "frontend"
          image_pull_policy = "Always"
          port {
            container_port = 80
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
    namespace = "performance-testing"
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
                number = 3000
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
                number = 3000
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
                number = 5000
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
                number = 80
              }
            }
          }
        }
      }
    }
  }
}
