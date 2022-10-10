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