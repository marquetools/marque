<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?><?schematron-phases phaseids="VALUECHECK"?><!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       --><sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00490">
   <sch:p xmlns:ism="urn:us:gov:ic:ism"
          ism:classification="U"
          ism:ownerProducer="USA"
          class="ruleText">
      [ISM-ID-00490][Error] Use of the Enterprise Role Restrictive access policy requires an Enterprise Role vocabulary type.
   </sch:p>
   <sch:p xmlns:ism="urn:us:gov:ic:ism"
          ism:classification="U"
          ism:ownerProducer="USA"
          class="codeDesc">
      If ntk:AccessProfile has an ntk:AccessPolicy = 'urn:us:gov:ic:aces:ntk:enterprise:role:restrictive', 
      then ntk:AccessProfileValue/@ntk:vocabulary MUST be 'urn:us:gov:ic:cvenum:role:enterprise:role'.
   </sch:p>
   <sch:rule id="ISM-ID-00490-R1"
             context="ntk:AccessProfile[ntk:AccessPolicy = 'urn:us:gov:ic:aces:ntk:enterprise:role:restrictive']/ntk:VocabularyType">
      <sch:assert test="@ntk:source = 'urn:us:gov:ic:cvenum:role:enterprise:role'"
                  flag="error"
                  role="error">
         [ISM-ID-00490][Error] Use of the Enterprise Role Restrictive access policy requires an Enterprise Role vocabulary type.
      </sch:assert>
   </sch:rule>
</sch:pattern>
