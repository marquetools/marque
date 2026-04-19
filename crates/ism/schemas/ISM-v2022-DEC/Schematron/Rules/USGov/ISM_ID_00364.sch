<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00364">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00364][Error] If an ISM_USGOV_RESOURCE has a value in @ism:compilationReason and @ism:noAggregation is present,
        @ism:noAggregation must be false.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If an ISM_USGOV_RESOURCE has a value in @ism:compilationReason and @ism:noAggregation is present,
        @ism:noAggregation must be false.
    </sch:p>
    <sch:rule id="ISM-ID-00364-R1" context="*[$ISM_USGOV_RESOURCE and string-length(normalize-space(@ism:compilationReason)) &gt; 0 and string-length(normalize-space(@ism:noAggregation)) &gt; 0]">
        <sch:assert test="@ism:noAggregation = false() " flag="error" role="error">
            [ISM-ID-00364][Error] If an ISM_USGOV_RESOURCE has a value in @ism:compilationReason and @ism:noAggregation is present,
            @ism:noAggregation must be false.
        </sch:assert>
    </sch:rule>
</sch:pattern>