<template>
  <div class="test-container">
    <div class="uk-overflow-auto test-table-container">
      <div class="uk-grid-small uk-child-width-1-5@s uk-text-center" uk-grid>
        <div class="uk-width-auto@m">
          <button class="uk-button uk-button-danger" type="button" @click="stopMe()">Stop</button>
        </div>
        <div class="uk-width-auto@m">
          <button class="uk-button uk-button-danger" type="button" @click="deleteMe()">Delete</button>
        </div>
        <div class="uk-width-auto@m">
          <button class="uk-button uk-button-primary" type="button" @click="restartMe()">Restart</button>
        </div>
        <div class="uk-width-auto@m">
          <button class="uk-button uk-button-primary" type="button" @click="downloadMe()">Download</button>
        </div>
        <div class="uk-width-auto@m">
          <button class="uk-button uk-button-primary" type="button" @click="showStats()">Stats</button>
        </div>
      </div>
      <table class="uk-table uk-table-small uk-table-striped uk-table-responsive">
        <thead>
          <tr>
            <th class="text">ID: {{ test.id }}</th>
            <th class="text">USERS: {{ test.info.users }}</th>
            <th class="text">SPAWN RATE: {{ test.info.spawn_rate }}</th>
            <th class="text">WORKERS: {{ test.info.workers ? test.info.workers : 0 }}</th>
            <th class="text">HOST: {{ test.info.host }}</th>
            <th class="text">TIME: {{ test.info.time ? test.info.time : 0 }} s</th>

            <th>
              <div v-if="test.status == 0" uk-spinner></div>
            </th>
          </tr>
        </thead>
      </table>
      <table class="uk-table uk-table-small uk-table-striped uk-table-responsive">
        <thead>
          <tr>
            <th class="text">Type</th>
            <th class="text">Name</th>
            <th class="text">Requests</th>
            <th class="text">Failures</th>
            <th class="text">Med Res Time</th>
            <th class="text">Avg Res Time</th>
            <th class="text">Min Res Time</th>
            <th class="text">Max Res Time</th>
            <th class="text">Avg Content Size</th>
            <th class="text">R/s</th>
            <th class="text">F/s</th>
          </tr>
        </thead>
        <tbody>
          <tr class="row" v-for="result in test.results" :key="result">
            <td class="text">{{ result.type }}</td>
            <td class="text">{{ result.name }}</td>
            <td class="text">{{ result.request_count }}</td>
            <td class="text">{{ result.failure_count }}</td>
            <td class="text">{{ result.median_response_time.slice(0, 6) }}</td>
            <td class="text">{{ result.avarage_response_time.slice(0, 6) }}</td>
            <td class="text">{{ result.min_response_time.slice(0, 6) }}</td>
            <td class="text">{{ result.max_response_time.slice(0, 6) }}</td>
            <td class="text">{{ result.avarage_content_size.slice(0, 6) }}</td>
            <td class="text">{{ result.requests_per_second.slice(0, 6) }}</td>
            <td class="text">{{ result.failures_per_seconde.slice(0, 6) }}</td>
          </tr>
        </tbody>
      </table>
    </div>
    <div v-if="showStatsBool" :id="test.id + '-chartContainer'" style="height: 370px; width: 100%"></div>
  </div>
</template>

<script>
export default {
  name: "Test",
  props: ["test", "darkTheme"],
  watch: {
    darkTheme: function (newVal, oldVal) {
      if (this.chart) {
        this.chart.options.theme = newVal ? "dark1" : "light1";
        this.chart.render();
      }
    },
    // test: {
    //   handler(newVal) {
    //     const lastHistory = newVal.last_history;
    //     this.updateChart(lastHistory);
    //   },
    //   deep: true,
    // },
  },
  data() {
    return {
      chart: null,
      total_median_response_time: {
        name: "Total Median Response Time",
        type: "line",
        showInLegend: true,
        dataPoints: [],
      },
      total_average_response_time: {
        name: "Total Average Response Time",
        type: "line",
        showInLegend: true,
        dataPoints: [],
      },
      total_min_response_time: {
        name: "Total Min Response Time",
        type: "line",
        showInLegend: true,
        dataPoints: [],
      },
      total_max_response_time: {
        name: "Total Max Response Time",
        type: "line",
        showInLegend: true,
        dataPoints: [],
      },
      showStatsBool: false,
    };
  },

  methods: {
    updateChart(lastHistory) {
      if (lastHistory != null) {
        const date = new Date(parseInt(lastHistory.timestamp) * 1000);
        this.total_median_response_time.dataPoints.push({
          x: date,
          y: parseInt(lastHistory.total_median_response_time),
        });
        this.total_average_response_time.dataPoints.push({
          x: date,
          y: parseInt(lastHistory.total_average_response_time),
        });
        this.total_min_response_time.dataPoints.push({
          x: date,
          y: parseInt(lastHistory.total_min_response_time),
        });
        this.total_max_response_time.dataPoints.push({
          x: date,
          y: parseInt(lastHistory.total_max_response_time),
        });
        this.chart.render();
      }
    },
    resetChartData(){
      this.total_median_response_time.dataPoints = [];
      this.total_average_response_time.dataPoints = [];
      this.total_min_response_time.dataPoints = [];
      this.total_max_response_time.dataPoints = [];
    },
    stopMe() {
      this.$emit("stop_me");
    },
    deleteMe() {
      this.$emit("delete_me");
    },
    restartMe() {
      this.$emit("restart_me");
    },
    downloadMe() {
      this.$emit("download_me");
    },
    showStats() {
      //get the data
      fetch(`/api/stats/${this.test.info.project_id}/${this.test.info.script_id}/${this.test.id}`)
        .then((data) => data.json())
        .then((data) => {
          this.showStatsBool = true;
          this.$nextTick(function () {
            this.resetChartData();  
            this.setupChart(data.content)
          })
        })
        .catch();

    },
    toggleDataSeries(e) {
      if (typeof e.dataSeries.visible === "undefined" || e.dataSeries.visible) {
        e.dataSeries.visible = false;
      } else {
        e.dataSeries.visible = true;
      }
      this.chart.render();
    },
    setupChart(history) {
      if (history != null && history.length > 0) {
        for (var i = 0; i < history.length; i++) {
          const record = history[i];
          const date = new Date(parseInt(record.timestamp) * 1000);
          this.total_median_response_time.dataPoints.push({
            x: date,
            y: parseInt(record.total_median_response_time),
          });
          this.total_average_response_time.dataPoints.push({
            x: date,
            y: parseInt(record.total_average_response_time),
          });
          this.total_min_response_time.dataPoints.push({
            x: date,
            y: parseInt(record.total_min_response_time),
          });
          this.total_max_response_time.dataPoints.push({
            x: date,
            y: parseInt(record.total_max_response_time),
          });
        }
      }
      var theme = "light1";
      if (this.darkTheme) theme = "dark1";

      this.chart = new CanvasJS.Chart(this.test.id + "-chartContainer", {
        animationEnabled: true,
        zoomEnabled: true,
        //exportEnabled: true,
        theme: theme,
        axisX: {
          gridThickness: 0,
          lineThickness: 1,
        },
        axisY: {
          gridThickness: 0,
          lineThickness: 1,
        },
        legend: {
          cursor: "pointer",
          fontSize: 16,
          itemclick: this.toggleDataSeries,
        },
        toolTip: {
          shared: true,
        },
        data: [
          this.total_median_response_time,
          this.total_average_response_time,
          this.total_min_response_time,
          this.total_max_response_time,
        ],
      });
      this.chart.render();
    },
  },
  mounted() {
    //this.setupChart(this.test.history); //this has been removed as it is really slow
  },
};
</script>

<style>

</style>