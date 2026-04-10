<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00533">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00533][Error] All resource elements with three or more @ism:SARIdentifier tokens will result in an error when @ism:compliesWith are 
        both DoD and IC.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If there are 3 or more SARs in the resource node and if ism:compliesWith contains both tokens [USIC] and [USDOD], then ERROR.
    </sch:p>
    <sch:rule id="ISM-ID-00533-R1" context="*[@ism:resourceElement='true' and @ism:SARIdentifier and $ISM_USDOD_RESOURCE and $ISM_USIC_RESOURCE]">
        <sch:assert test="util:countSARmarkings(./@ism:SARIdentifier) &lt; 3"
            flag="error" 
            role="error">
            [ISM-ID-00533][Error] All resource elements that contain @ism:compliesWith="USGov USDOD USIC" attribute MUST contain no more than two (2) tokens
            in @ism:SARIdentifier. This rule satisfies requirements specified in the IC and DoD authoritative sources for SAP policies; [1] DoD Directive 
            5205.07 - Special Access Program (SAP) Policy and [2] IC Markings System Register and Manual.
        </sch:assert>
    </sch:rule>
</sch:pattern>