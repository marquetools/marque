<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?><?schematron-phases phaseids="BANNER PORTION VALUECHECK"?><!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       --><sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00507">
    <sch:p xmlns:ism="urn:us:gov:ic:ism"
          ism:classification="U"
          ism:ownerProducer="USA"
          class="ruleText">
        [ISM-ID-00507][Error] If (ISM_USCUI_RESOURCE or ISM_USCUIONLY_RESOURCE) and attribute @ism:disseminationControls
        contains one or more of the name tokens [AC] or [AWP], then attribute @ism:cuiBasic
        must contain the name token [PRIVILEGE].
        
        Human Readable: A CUI document containing one of the CUI limited dissemination controls [AC] or [AWP] must be marked
        with the CUI Basic Category of [PRIVILEGE].
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism"
          ism:classification="U"
          ism:ownerProducer="USA"
          class="codeDesc">
        If the document is an (ISM_USCUI_RESOURCE or ISM_USCUIONLY_RESOURCE), for each element which
        specifies attribute @ism:disseminationControls contains one or more of the name tokens [AC] or [AWP], 
        then attribute @ism:cuiBasic must contain the name token [PRIVILEGE].
    </sch:p>
    <sch:rule id="ISM-ID-00507-R1"
             context="*[($ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE) and              util:containsAnyOfTheTokens(@ism:disseminationControls, ('AC','AWP'))]">
          <sch:assert test="util:containsAnyOfTheTokens(@ism:cuiBasic, ('PRIVILEGE'))"
                  flag="error"
                  role="error">
              [ISM-ID-00507][Error] If (ISM_USCUI_RESOURCE or ISM_USCUIONLY_RESOURCE) and attribute @ism:disseminationControls
              contains one or more of the name tokens [AC] or [AWP], then attribute @ism:cuiBasic
              must contain the name token [PRIVILEGE].
              
              Human Readable: A CUI document marked one or more of [AC] Attorney-Client and/or [AWP] Attorney Work Product
              must be marked with the CUI Basic Category Marking of PRIVILEGE.
        </sch:assert>
    </sch:rule>
</sch:pattern>
