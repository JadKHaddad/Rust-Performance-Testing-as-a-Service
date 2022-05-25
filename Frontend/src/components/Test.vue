<template>
  <div class="test-container">
    <div class="uk-overflow-auto">
      <table
        class="uk-table uk-table-small uk-table-striped uk-table-responsive"
      >
        <thead>
          <tr>
            <th>{{ test.id }}</th>
            <th>{{ test.info.users }}</th>
            <th>{{ test.info.spawn_rate }}</th>
            <th>{{ test.info.workers }}</th>
            <th>{{ test.info.host }}</th>
            <th>{{ test.info.time }}</th>
            <th></th>
            <th></th>
            <th></th>
            <th></th>
            <th><div v-if="test.status == 0" uk-spinner></div></th>
          </tr>
        </thead>
        <thead>
          <tr>
            <th>Type</th>
            <th>Name</th>
            <th>Requests</th>
            <th>Failures</th>
            <th>Med Res Time</th>
            <th>Avg Res Time</th>
            <th>Min Res Time</th>
            <th>Max Res Time</th>
            <th>Avg Content Size</th>
            <th>Requests/s</th>
            <th>Failures/s</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="result in test.results" :key="result">
            <td>{{ result.type }}</td>
            <td>{{ result.name }}</td>
            <td>{{ result.request_count }}</td>
            <td>{{ result.failure_count }}</td>
            <td>{{ result.median_response_time.slice(0, 6) }}</td>
            <td>{{ result.avarage_response_time.slice(0, 6) }}</td>
            <td>{{ result.min_response_time.slice(0, 6) }}</td>
            <td>{{ result.max_response_time.slice(0, 6) }}</td>
            <td>{{ result.avarage_content_size.slice(0, 6) }}</td>
            <td>{{ result.requests_per_second.slice(0, 6) }}</td>
            <td>{{ result.failures_per_seconde.slice(0, 6) }}</td>
          </tr>
        </tbody>
      </table>
    </div>

    <br />
    <br />
    <br />
    <div
      :id="test.id + '-chartContainer'"
      style="height: 370px; width: 100%"
    ></div>
    <br />
    <br />
    <br />
  </div>
</template>

<script>
export default {
  name: "Test",
  props: ["test"],
  watch: {
    test: {
      handler(newVal) {
        const lastHistory = newVal.last_history;
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
      deep: true,
    },
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
    };
  },

  methods: {
    toggleDataSeries(e) {
      if (typeof e.dataSeries.visible === "undefined" || e.dataSeries.visible) {
        e.dataSeries.visible = false;
      } else {
        e.dataSeries.visible = true;
      }
      this.chart.render();
    },
    setupChart() {
      if (this.test.history != null && this.test.history.length > 0) {
        for (var i = 0; i < this.test.history.length; i++) {
          const record = this.test.history[i];
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
      this.chart = new CanvasJS.Chart(this.test.id + "-chartContainer", {
        animationEnabled: true,
        zoomEnabled: true,
        exportEnabled: true,
        //theme: "dark2",
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
    this.setupChart();
    /*
    setInterval(function () {
      data1.dataPoints.push({
        x: data1.dataPoints[data1.dataPoints.length - 1].x + 1,
        y:
          data1.dataPoints[data1.dataPoints.length - 1].y +
          (Math.random() * 10 - 5),
      });
      data2.dataPoints.push({
        x: data2.dataPoints[data2.dataPoints.length - 1].x + 1,
        y:
          data2.dataPoints[data2.dataPoints.length - 1].y +
          (Math.random() * 10 - 5),
      });
      data3.dataPoints.push({
        x: data3.dataPoints[data3.dataPoints.length - 1].x + 1,
        y:
          data3.dataPoints[data3.dataPoints.length - 1].y +
          (Math.random() * 10 - 5),
      });
      chart.render();
    }, 2000);
    */
  },
};
</script>

<style>
</style>