locals {
  root_path_tmp = "/${replace(abspath(path.root), ":", "")}"
  root_path     = replace(local.root_path_tmp, "////", "/")
  is_linux      = length(regexall("/home/", lower(abspath(path.root)))) > 0
  paths = {
    context_path        = "../../"
    dockerfiles_path    = "Dockerfiles"
    data_path           = "${local.root_path}/../../Dockerfiles/Performance-Testing-Data"
    container_data_path = "/home/app/Backend/Performance-Testing-Data"
  }
  ports = {
    master_internal_port       = 3000
    master_external_port       = 3000
    worker_internal_port       = 5000
    worker_external_port       = 5000
    frontend_internal_port     = 80
    frontend_external_port     = 7000
    loadbalancer_internal_port = 80
    loadbalancer_external_port = 5000
    entrypoint_internal_port   = 80
    entrypoint_external_port   = 8000
  }
  docker_registry          = "localhost:32000"
  container_restart_policy = "unless-stopped"
}