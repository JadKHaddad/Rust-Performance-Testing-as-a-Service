resource "kubernetes_namespace" "main_namespace" {
  metadata {
    name = var.namespace
  }
}
