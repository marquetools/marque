<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00460">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00460][Error] If 1. Any attribute @ism:noticeType contains [ITAR-EAR]
        AND 
        2. Attribute @ism:noticeType of ISM_RESOURCE_ELEMENT contains [DoD-Dist-A]
        
        Human Readable: All documents that include a DoD 5230.24 ITAR-EAR notice MUST NOT
        have a DoD-Dist-A distribution statement for the entire document.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document contains an [ITAR-EAR] notice and the current element is the ISM_RESOURCE_ELEMENT, 
        this rule ensures that there is NO attribute @ism:noticeType with a value of [DoD-Dist-A].
    </sch:p>
    <sch:rule id="ISM-ID-00460-R1" context="*[generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:noticeType, ('ITAR-EAR')) ]">
      <sch:assert test="not(util:containsAnyOfTheTokens(@ism:noticeType, ('DoD-Dist-A')))" flag="error" role="error">
          [ISM-ID-00460][Error] If 1. Any attribute @ism:noticeType contains [ITAR-EAR]
          AND 
          2. Attribute @ism:noticeType of ISM_RESOURCE_ELEMENT contains [DoD-Dist-A]
          
          Human Readable: All documents that include a DoD 5230.24 ITAR-EAR notice MUST NOT
          have a DoD-Dist-A distribution statement for the entire document.
        </sch:assert>
    </sch:rule>
</sch:pattern>