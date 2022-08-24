# Performace Testing as a Service
Built to run on Kubernetes, this service is designed to test the performance of given applications using locust.
## TODO
Locust does not stop if host is not valid<br>
Remove "Unwraps"<br>
Other features..<br>
workers and worker-info should be stored in redis: if master dies or workers die. No need to register workers, save workers in redis instead<br>
MASTER and WORKER: recovery thread to recover redis lost data if redis dies