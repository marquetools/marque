<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?><?schematron-phases phaseids="BANNER PORTION STRUCTURECHECK"?><!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00495">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA"
        class="ruleText"> [ISM-ID-00495][Error] If @ism:compliesWith="USA-CUI-ONLY" then attributes
        @ism:classification and @ism:ownerProducer must not be specified. </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA"
        class="codeDesc"> If the document has @ism:compliesWith="USA-CUI-ONLY", as defined in
        variable ISM_USCUIONLY_RESOURCE, this rule ensures that NONE of the following attributes are
        specified: @ism:classification and @ism:ownerProducer. </sch:p>
    <sch:rule id="ISM-ID-00495-R1" context="*[@ism:* and $ISM_USCUIONLY_RESOURCE]">
        <sch:assert test="not(@ism:classification or @ism:ownerProducer)" flag="error" role="error">
            [ISM-ID-00495][Error] If @ism:compliesWith="USA-CUI-ONLY" then attributes
            @ism:classification and @ism:ownerProducer must not be specified. </sch:assert>
    </sch:rule>
</sch:pattern>
