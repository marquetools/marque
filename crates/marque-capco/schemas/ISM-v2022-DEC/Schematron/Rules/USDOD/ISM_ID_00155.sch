<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00155">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00155][Error] If ISM_USDOD_RESOURCE and 
        1. not ISM_DOD_DISTRO_EXEMPT
        AND
        2. Attribute @ism:noticeType of ISM_RESOURCE_ELEMENT does not contain one of 
        [DoD-Dist-A], [DoD-Dist-B], [DoD-Dist-C], [DoD-Dist-D], [DoD-Dist-E], or [DoD-Dist-F]
        
        Human Readable: All US DOD documents that do not claim exemption from 
        DoD5230.24 distribution statements must have a distribution statement
        for the entire document.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USDOD_RESOURCE and not ISM_DOD_DISTRO_EXEMPT and
        the current element is the ISM_RESOURCE_ELEMENT, this rule ensures that 
        attribute @ism:noticeType is specified with a value containing one of the
        tokens: [DoD-Dist-A], [DoD-Dist-B], [DoD-Dist-C], [DoD-Dist-D], [DoD-Dist-E], [DoD-Dist-F].
    </sch:p>
  <sch:rule id="ISM-ID-00155-R1" context="*[$ISM_USDOD_RESOURCE and not($ISM_DOD_DISTRO_EXEMPT) and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)]">
        <sch:assert test="util:containsAnyOfTheTokens(@ism:noticeType, ('DoD-Dist-A', 'DoD-Dist-B', 'DoD-Dist-C', 'DoD-Dist-D', 'DoD-Dist-E', 'DoD-Dist-F'))" flag="error" role="error">
            [ISM-ID-00155][Error] If ISM_USDOD_RESOURCE and 
            1. not ISM_DOD_DISTRO_EXEMPT
            AND
            2. Attribute @ism:noticeType of ISM_RESOURCE_ELEMENT does not contain one of 
            [DoD-Dist-A], [DoD-Dist-B], [DoD-Dist-C], [DoD-Dist-D], [DoD-Dist-E], or [DoD-Dist-F]
            
            Human Readable: All US DOD documents that do not claim exemption from 
            DoD5230.24 distribution statements must have a distribution statement
            for the entire document.
        </sch:assert>
    </sch:rule>
</sch:pattern>