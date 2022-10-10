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
      app = "worker-${count.index + 1}"
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