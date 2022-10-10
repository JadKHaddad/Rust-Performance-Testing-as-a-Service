resource "docker_image" "master" {
  depends_on = [
    docker_image.runner,
    docker_image.builder
  ]
  name         = "${var.registry}/master-release:latest"
  keep_locally = true
  build {
    path       = local.paths.image.context_path
    dockerfile = "${local.paths.image.dockerfiles_path}/Dockerfile.master-release"
  }
  provisioner "local-exec" {
    command = "docker image push ${self.name}"
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
          image             = docker_image.master.name
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