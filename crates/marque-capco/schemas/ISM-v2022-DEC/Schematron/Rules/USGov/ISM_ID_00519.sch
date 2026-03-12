<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="PORTION STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00519">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00519][Error] For ism:Notice or ism:NoticeExternal, if @ism:noticeProseID is absent then ism:NoticeText is required.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        This rule ensures that for ism:Notice or ism:NoticeExternal, if @ism:noticeProseID is absent then ism:NoticeText is required.
    </sch:p>
    <sch:rule id="ISM-ID-00519-R1" context="ism:Notice[$ISM_USGOV_RESOURCE and not(exists(@ism:noticeProseID))] | ism:NoticeExternal[$ISM_USGOV_RESOURCE and not(exists(@ism:noticeProseID))]">
        <sch:assert test="exists(.//ism:NoticeText)" flag="error" role="error">
            [ISM-ID-00519][Error] For ism:Notice or ism:NoticeExternal, if @ism:noticeProseID is absent then ism:NoticeText is required.
        </sch:assert>
    </sch:rule>
</sch:pattern>