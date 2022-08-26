# Performace Testing as a Service
Built to run on Kubernetes, this service is designed to test the performance of given applications using locust

## Deployment using MicroK8s

* Use the project's root directory as your working directory

* Enable the necessary MicroK8s oddons
```sh
microk8s enable dns registry ingress metallb
```
* MicroK8s' default local registry's port is, ```32000```

* Build the helper images
```sh
docker build -t builder:latest -f Dockerfiles/Dockerfile.builder .
docker build -t runner:latest -f Dockerfiles/Dockerfile.runner .
```
* Build the images and push them to the local registry
```sh
docker build -t localhost:32000/master-release:latest -f Dockerfiles/Dockerfile.master-release .
docker push localhost:32000/master-release:latest
```
```sh
docker build -t localhost:32000/worker-release:latest -f Dockerfiles/Dockerfile.worker-release .
docker push localhost:32000/worker-release:latest
```
```sh
docker build -t localhost:32000/frontend:latest -f Dockerfiles/Dockerfile.frontend .
docker push localhost:32000/frontend:latest
```
* Create the namespace
```sh
kubectl create namespace performance-testing
```
* Make things easier by setting kubectl's default namespace
```sh
kubectl config set-context --current --namespace=performance-testing
```
* Apply kubernetes yaml files
```sh
kubectl apply -f Kubernetes/pv-volume.yaml
kubectl apply -f Kubernetes/pv-claim.yaml
kubectl apply -f Kubernetes/configmap.yaml
kubectl apply -f Kubernetes/redis.yaml
kubectl apply -f Kubernetes/frontend.yaml
kubectl apply -f Kubernetes/master.yaml
kubectl apply -f Kubernetes/worker-1.yaml
kubectl apply -f Kubernetes/worker-2.yaml
kubectl apply -f Kubernetes/worker-loadbalancer.yaml
kubectl apply -f Kubernetes/ingress.yaml
```
* Add more workers by using the WorkerCreator
```sh
python3 Kubernetes/WorkerCreator/app.py 3

kubectl apply -f Kubernetes/worker-3.yaml
```
* Depending on your use case and kubernetes distribution, you may edit ```Kubernetes/pv-volume.yaml``` and ```Kubernetes/ingress.yaml```

* MicroK8s's default ingress' port is, ```80```

* After building new images, reapply the deployments
```sh
# kubectl rollout restart -n <namespace> deployment <deployment>

kubectl rollout restart deployment -n performance-testing frontend-deployment
kubectl rollout restart deployment -n performance-testing master-deployment
kubectl rollout restart deployment -n performance-testing worker-1-deployment
kubectl rollout restart deployment -n performance-testing worker-2-deployment

...
```
## Contributors
* Jad K. Haddad <jadkhaddad@gmail.com>

## License & copyright
© 2022 Jad K. Haddad
Licensed under the [MIT License](LICENSE)

## TODO
* Locust does not stop if host is not valid
* Locust workers
* MASTER, WORKER: create for threads one red connection that reconnects on error
* Other features..
