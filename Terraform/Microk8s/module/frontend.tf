resource "docker_image" "frontend" {
  name         = "${var.registry}/frontend:latest"
  keep_locally = true
  build {
    path       = local.paths.image.context_path
    dockerfile = "${local.paths.image.dockerfiles_path}/Dockerfile.frontend"
  }
  provisioner "local-exec" {
    command = "docker image push ${self.name}"
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
          image             = docker_image.frontend.image_id
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
