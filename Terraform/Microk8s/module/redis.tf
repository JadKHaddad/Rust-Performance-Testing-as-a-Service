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