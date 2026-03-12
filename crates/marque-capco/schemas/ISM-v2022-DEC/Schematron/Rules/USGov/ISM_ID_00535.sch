<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00535">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00535][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:disseminationControls contains the name token [WAIVED], then 
        attribute @ism:compliesWith must contain [USDOD]. 
        Human Readable: USA documents containing the WAIVED dissemination control must comply with USDOD rules.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USGOV_RESOURCE, for each element which has 
    	attribute @ism:disseminationControls specified with a value containing
    	the token [WAIVED], this rule ensures that attribute @ism:compliesWith contains [USDOD].
    </sch:p>
    <sch:rule id="ISM-ID-00535-R1" context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('WAIVED'))]">
        <sch:assert test="$ISM_USDOD_RESOURCE" flag="error" role="error">
            [ISM-ID-00535][Error] If ISM_USGOV_RESOURCE and attribute 
            @ism:disseminationControls contains the name token [WAIVED], then 
            attribute @ism:compliesWith must contain [USDOD]. 
            Human Readable: USA documents containing the WAIVED 
            dissemination control must comply with USDOD rules.
        </sch:assert>
    </sch:rule>
</sch:pattern>